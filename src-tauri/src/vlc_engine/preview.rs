//! Captura de frames fora da reprodução: prévia do hover da barra e capa da
//! coleção.
//!
//! Usa um MediaPlayer SEM janela: o libVLC decodifica direto para um buffer
//! nosso (`libvlc_video_set_callbacks`, chroma RV32 = BGRA), então dá para
//! buscar um instante qualquer e capturar o frame sem tocar na reprodução em
//! andamento. Os objetos libVLC não são `Send` → thread dedicada + canal,
//! mesmo padrão de `engine.rs`.
//!
//! Memória: a instância da prévia só nasce no primeiro hover e é destruída ao
//! fechar o player; a da capa vive só durante a captura.

use std::cell::UnsafeCell;
use std::ffi::{c_void, CString};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use vlc::{sys, Instance, Media, MediaPlayer, MediaPlayerAudioEx};

use crate::constants::{
    AUDIO_OUTPUT_DUMMY, POSTER_HEIGHT, POSTER_WIDTH, PREVIEW_CHROMA, PREVIEW_FRAME_TIMEOUT_MS,
    PREVIEW_HEIGHT, PREVIEW_POLL_MS, PREVIEW_WIDTH, TRACK_DISABLED_ID,
};

const BYTES_PER_PIXEL: u32 = 4;
const MS_PER_SEC: f64 = 1000.0;

/// Buffer onde o libVLC escreve o frame decodificado, mais um contador de
/// frames exibidos (a thread de vídeo do libVLC é quem escreve).
struct FrameSink {
    buf: UnsafeCell<Vec<u8>>,
    frames: AtomicU64,
}

// SAFETY: só a thread de vídeo do libVLC escreve no buffer, e a leitura ocorre
// depois de pausar o player, quando o contador de frames parou de avançar.
unsafe impl Sync for FrameSink {}
unsafe impl Send for FrameSink {}

unsafe extern "C" fn lock_cb(opaque: *mut c_void, planes: *mut c_void) -> *mut c_void {
    let sink = &*(opaque as *const FrameSink);
    let planes = planes as *mut *mut c_void;
    *planes = (*sink.buf.get()).as_mut_ptr() as *mut c_void;
    null_mut()
}

unsafe extern "C" fn unlock_cb(_opaque: *mut c_void, _picture: *mut c_void, _planes: *const *mut c_void) {}

unsafe extern "C" fn display_cb(opaque: *mut c_void, _picture: *mut c_void) {
    let sink = &*(opaque as *const FrameSink);
    sink.frames.fetch_add(1, Ordering::SeqCst);
}

/// Player headless numa resolução fixa. Ordem dos campos = ordem de drop:
/// o player é liberado antes da instância e do buffer que ele escreve.
struct Grabber {
    mdp: MediaPlayer,
    instance: Instance,
    sink: Arc<FrameSink>,
    width: u32,
    height: u32,
    loaded: Option<String>,
}

impl Grabber {
    fn new(width: u32, height: u32) -> Option<Self> {
        let sink = Arc::new(FrameSink {
            buf: UnsafeCell::new(vec![0u8; (width * height * BYTES_PER_PIXEL) as usize]),
            frames: AtomicU64::new(0),
        });
        let instance = Instance::new()?;
        let mdp = MediaPlayer::new(&instance)?;
        unsafe {
            let chroma = CString::new(PREVIEW_CHROMA).ok()?;
            sys::libvlc_video_set_format(mdp.raw(), chroma.as_ptr(), width, height, width * BYTES_PER_PIXEL);
            sys::libvlc_video_set_callbacks(
                mdp.raw(),
                Some(lock_cb),
                Some(unlock_cb),
                Some(display_cb),
                Arc::as_ptr(&sink) as *mut c_void,
            );
            // NÃO usar set_mute aqui: no Windows o libVLC aplica mute na sessão
            // de áudio do PROCESSO, que é compartilhada — mutaria a reprodução
            // principal junto. A prévia manda o áudio para o dispositivo nulo e
            // nem seleciona trilha de áudio.
            let dummy = CString::new(AUDIO_OUTPUT_DUMMY).ok()?;
            sys::libvlc_audio_output_set(mdp.raw(), dummy.as_ptr());
            sys::libvlc_audio_set_track(mdp.raw(), TRACK_DISABLED_ID);
        }
        Some(Self { mdp, instance, sink, width, height, loaded: None })
    }

    fn load(&mut self, path: &str) -> Option<()> {
        if self.loaded.as_deref() == Some(path) {
            return Some(());
        }
        self.mdp.stop();
        let media = Media::new_path(&self.instance, path)?;
        self.mdp.set_media(&media);
        self.loaded = Some(path.to_string());
        Some(())
    }

    /// Busca o instante, espera o frame e devolve o PNG.
    fn grab(&mut self, path: &str, seconds: f64) -> Option<Vec<u8>> {
        self.load(path)?;
        let capture_seconds = self
            .mdp
            .get_media()
            .and_then(|media| media.duration())
            .filter(|duration| *duration > 0)
            .map(|duration| seconds.min((duration as f64 / MS_PER_SEC - 1.0).max(0.0)))
            .unwrap_or(seconds);
        let before = self.sink.frames.load(Ordering::SeqCst);
        self.mdp.set_pause(false);
        if !self.mdp.is_playing() {
            self.mdp.play().ok()?;
        }
        unsafe {
            // Legenda fora do frame capturado.
            sys::libvlc_video_set_spu(self.mdp.raw(), TRACK_DISABLED_ID);
        }
        self.mdp.set_time((capture_seconds * MS_PER_SEC) as i64);

        // Ignora o primeiro frame: pode ser o anterior ao seek.
        let deadline = Instant::now() + Duration::from_millis(PREVIEW_FRAME_TIMEOUT_MS);
        while self.sink.frames.load(Ordering::SeqCst) < before + 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(PREVIEW_POLL_MS));
        }
        let got_frame = self.sink.frames.load(Ordering::SeqCst) > before;
        self.mdp.set_pause(true);
        if !got_frame {
            return None;
        }

        // SAFETY: player pausado — o libVLC não está mais escrevendo no buffer.
        let bgra = unsafe { (*self.sink.buf.get()).clone() };
        encode_png(&bgra, self.width, self.height)
    }
}

enum PreviewCmd {
    /// Só registra o arquivo alvo da prévia — não inicializa nada.
    Open(String),
    Grab { seconds: f64, reply: Sender<Option<String>> },
    /// Capa/miniatura: instância própria na resolução pedida, descartada no fim.
    GrabPoster { path: String, seconds: f64, width: u32, height: u32, reply: Sender<Option<Vec<u8>>> },
    /// Libera a instância libVLC da prévia.
    Close,
}

/// Lado público (Send + Sync) do capturador de frames.
pub struct PreviewHandle {
    tx: Mutex<Sender<PreviewCmd>>,
}

impl PreviewHandle {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<PreviewCmd>();
        thread::spawn(move || {
            let mut preview: Option<Grabber> = None;
            let mut target: Option<String> = None;

            for cmd in rx {
                match cmd {
                    PreviewCmd::Open(path) => target = Some(path),
                    PreviewCmd::Grab { seconds, reply } => {
                        let frame = (|| {
                            let path = target.clone()?;
                            if preview.is_none() {
                                preview = Grabber::new(PREVIEW_WIDTH, PREVIEW_HEIGHT);
                            }
                            let png = preview.as_mut()?.grab(&path, seconds)?;
                            Some(to_data_uri(&png))
                        })();
                        let _ = reply.send(frame);
                    }
                    PreviewCmd::GrabPoster { path, seconds, width, height, reply } => {
                        let png = Grabber::new(width, height).and_then(|mut g| {
                            g.grab(&path, seconds).or_else(|| {
                                if seconds > 0.0 {
                                    g.grab(&path, 0.0)
                                } else {
                                    None
                                }
                            })
                        });
                        let _ = reply.send(png);
                    }
                    PreviewCmd::Close => {
                        preview = None; // libera a instância libVLC da prévia
                        target = None;
                    }
                }
            }
        });
        Self { tx: Mutex::new(tx) }
    }

    fn send(&self, cmd: PreviewCmd) -> Result<(), String> {
        self.tx
            .lock()
            .map_err(|_| "prévia: lock envenenado".to_string())?
            .send(cmd)
            .map_err(|_| "prévia: thread encerrou".to_string())
    }

    /// Registra o arquivo da reprodução atual (não inicializa o libVLC).
    pub fn open(&self, path: &str) {
        let _ = self.send(PreviewCmd::Open(path.to_string()));
    }

    /// Frame da prévia como data URI (`data:image/png;base64,...`).
    pub fn grab(&self, seconds: f64) -> Option<String> {
        let (reply, rx) = mpsc::channel();
        self.send(PreviewCmd::Grab { seconds, reply }).ok()?;
        rx.recv().ok().flatten()
    }

    /// PNG da capa, em resolução maior, para gravar em disco.
    pub fn grab_poster(&self, path: &str, seconds: f64) -> Option<Vec<u8>> {
        self.grab_still(path, seconds, POSTER_WIDTH, POSTER_HEIGHT)
    }

    /// PNG avulso numa resolução qualquer (capa, miniatura de episódio…).
    pub fn grab_still(&self, path: &str, seconds: f64, width: u32, height: u32) -> Option<Vec<u8>> {
        let (reply, rx) = mpsc::channel();
        self.send(PreviewCmd::GrabPoster { path: path.to_string(), seconds, width, height, reply })
            .ok()?;
        rx.recv().ok().flatten()
    }

    pub fn close(&self) {
        let _ = self.send(PreviewCmd::Close);
    }
}

fn encode_png(bgra: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut rgba = Vec::with_capacity(bgra.len());
    for px in bgra.chunks_exact(BYTES_PER_PIXEL as usize) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], 0xFF]);
    }
    let img = image::RgbaImage::from_raw(width, height, rgba)?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some(png)
}

pub fn to_data_uri(png: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}
