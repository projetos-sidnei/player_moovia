//! Engine de reprodução (spec §8). Os objetos libVLC não são `Send`, então uma
//! thread dedicada é dona de todos eles; o resto do app fala com ela por canal.

use std::ffi::{c_void, CStr, CString};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::thread;

use serde::Serialize;
use tokio::sync::oneshot;
use vlc::{sys, Instance, Media, MediaPlayer, MediaPlayerAudioEx};

const MS_PER_SEC: f64 = 1000.0;
const MICROS_PER_MS: i64 = 1000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub id: i32,
    pub name: String,
}

/// Trilhas de áudio/legenda disponíveis + selecionadas (id -1 = desativado).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackMenu {
    pub audio: Vec<TrackInfo>,
    pub audio_current: i32,
    pub subtitles: Vec<TrackInfo>,
    pub subtitle_current: i32,
}

/// Percorre e LIBERA a lista encadeada de descrições do libvlc.
/// (o wrapper do vlc-rs tem off-by-one e perderia a última trilha)
unsafe fn collect_tracks(head: *mut sys::libvlc_track_description_t) -> Vec<TrackInfo> {
    let mut out = Vec::new();
    let mut p = head;
    while !p.is_null() {
        let name = if (*p).psz_name.is_null() {
            String::new()
        } else {
            CStr::from_ptr((*p).psz_name).to_string_lossy().into_owned()
        };
        out.push(TrackInfo { id: (*p).i_id, name });
        p = (*p).p_next;
    }
    if !head.is_null() {
        sys::libvlc_track_description_list_release(head);
    }
    out
}

pub enum PlayerCmd {
    Play {
        path: String,
        start_at_seconds: f64,
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetPause(bool),
    TogglePause,
    Seek(f64),
    GetPosition(oneshot::Sender<Option<f64>>),
    GetDuration(oneshot::Sender<Option<f64>>),
    SetVolume(i32),
    GetTracks(oneshot::Sender<TrackMenu>),
    AudioDebug(oneshot::Sender<String>),
    /// A mídia chegou ao fim (usado para emendar o próximo episódio).
    HasEnded(oneshot::Sender<bool>),
    SetRate(f32),
    /// Atraso do áudio em milissegundos (positivo = áudio depois do vídeo).
    SetAudioDelay(i64),
    SetSubtitleDelay(i64),
    /// Carrega um arquivo de legenda externo (.srt ao lado do vídeo).
    AddSubtitleFile(String),
    SetAudioTrack(i32),
    SetSubtitleTrack(i32),
    Stop,
}

/// Lado público (Send + Sync) do player — injetado como Tauri `State`.
pub struct PlayerHandle {
    tx: Mutex<Sender<PlayerCmd>>,
}

impl PlayerHandle {
    /// `hwnd` é o handle nativo da janela principal (render embutido, spec §8).
    pub fn spawn(hwnd: isize) -> Self {
        let (tx, rx) = mpsc::channel::<PlayerCmd>();
        thread::spawn(move || {
            let engine: Result<(Instance, MediaPlayer), String> = (|| {
                let instance = Instance::new().ok_or_else(|| {
                    "libVLC: falha ao inicializar (verifique VLC 64-bit instalado e o VLC_DIR do .env)"
                        .to_string()
                })?;
                let mdp = MediaPlayer::new(&instance)
                    .ok_or_else(|| "libVLC: falha ao criar MediaPlayer".to_string())?;
                Ok((instance, mdp))
            })();

            let (instance, mdp) = match engine {
                Ok(pair) => pair,
                Err(msg) => {
                    // Thread fica viva devolvendo o erro real — canal nunca "morre".
                    eprintln!("{msg}");
                    for cmd in rx {
                        match cmd {
                            PlayerCmd::Play { reply, .. } => {
                                let _ = reply.send(Err(msg.clone()));
                            }
                            PlayerCmd::GetPosition(reply) => {
                                let _ = reply.send(None);
                            }
                            PlayerCmd::GetDuration(reply) => {
                                let _ = reply.send(None);
                            }
                            PlayerCmd::GetTracks(reply) => {
                                let _ = reply.send(TrackMenu::default());
                            }
                            PlayerCmd::AudioDebug(reply) => {
                                let _ = reply.send(msg.clone());
                            }
                            PlayerCmd::HasEnded(reply) => {
                                let _ = reply.send(false);
                            }
                            _ => {}
                        }
                    }
                    return;
                }
            };
            mdp.set_hwnd(hwnd as *mut c_void);

            for cmd in rx {
                match cmd {
                    PlayerCmd::Play { path, start_at_seconds, reply } => {
                        let result = match Media::new_path(&instance, &path) {
                            Some(md) => {
                                mdp.set_media(&md);
                                match mdp.play() {
                                    Ok(()) => {
                                        // Cura instalações onde a sessão de áudio do processo
                                        // ficou muda (bug antigo da prévia): o Windows lembra
                                        // o mute por aplicativo entre execuções.
                                        unsafe { sys::libvlc_audio_set_mute(mdp.raw(), 0) };
                                        if start_at_seconds > 0.0 {
                                            mdp.set_time((start_at_seconds * MS_PER_SEC) as i64);
                                        }
                                        Ok(())
                                    }
                                    Err(()) => Err(format!("libVLC não conseguiu reproduzir: {path}")),
                                }
                            }
                            None => Err(format!("libVLC não abriu o arquivo: {path}")),
                        };
                        let _ = reply.send(result);
                    }
                    PlayerCmd::SetPause(paused) => mdp.set_pause(paused),
                    PlayerCmd::TogglePause => mdp.pause(),
                    PlayerCmd::Seek(seconds) => mdp.set_time((seconds * MS_PER_SEC) as i64),
                    PlayerCmd::GetPosition(reply) => {
                        let _ = reply.send(mdp.get_time().map(|ms| ms as f64 / MS_PER_SEC));
                    }
                    PlayerCmd::GetDuration(reply) => {
                        let dur = mdp
                            .get_media()
                            .and_then(|m| m.duration())
                            .filter(|&ms| ms > 0)
                            .map(|ms| ms as f64 / MS_PER_SEC);
                        let _ = reply.send(dur);
                    }
                    PlayerCmd::SetVolume(volume) => {
                        let _ = mdp.set_volume(volume);
                    }
                    PlayerCmd::HasEnded(reply) => {
                        let _ = reply.send(matches!(mdp.state(), vlc::State::Ended));
                    }
                    PlayerCmd::SetRate(rate) => unsafe {
                        sys::libvlc_media_player_set_rate(mdp.raw(), rate);
                    },
                    PlayerCmd::SetAudioDelay(ms) => unsafe {
                        sys::libvlc_audio_set_delay(mdp.raw(), ms * MICROS_PER_MS);
                    },
                    PlayerCmd::SetSubtitleDelay(ms) => unsafe {
                        sys::libvlc_video_set_spu_delay(mdp.raw(), ms * MICROS_PER_MS);
                    },
                    PlayerCmd::AddSubtitleFile(path) => unsafe {
                        if let Ok(c) = CString::new(path) {
                            sys::libvlc_video_set_subtitle_file(mdp.raw(), c.as_ptr());
                        }
                    },
                    PlayerCmd::AudioDebug(reply) => {
                        let info = unsafe {
                            format!(
                                "tocando={} pos={:?} trilha={} trilhas_disponiveis={} volume={} mute={}",
                                mdp.is_playing(),
                                mdp.get_time(),
                                sys::libvlc_audio_get_track(mdp.raw()),
                                sys::libvlc_audio_get_track_count(mdp.raw()),
                                sys::libvlc_audio_get_volume(mdp.raw()),
                                sys::libvlc_audio_get_mute(mdp.raw()),
                            )
                        };
                        let _ = reply.send(info);
                    }
                    PlayerCmd::GetTracks(reply) => {
                        let raw = mdp.raw();
                        let menu = unsafe {
                            TrackMenu {
                                audio: collect_tracks(sys::libvlc_audio_get_track_description(raw)),
                                audio_current: sys::libvlc_audio_get_track(raw),
                                subtitles: collect_tracks(sys::libvlc_video_get_spu_description(raw)),
                                subtitle_current: sys::libvlc_video_get_spu(raw),
                            }
                        };
                        let _ = reply.send(menu);
                    }
                    PlayerCmd::SetAudioTrack(id) => unsafe {
                        sys::libvlc_audio_set_track(mdp.raw(), id);
                    },
                    PlayerCmd::SetSubtitleTrack(id) => unsafe {
                        sys::libvlc_video_set_spu(mdp.raw(), id);
                    },
                    PlayerCmd::Stop => mdp.stop(),
                }
            }
        });
        Self { tx: Mutex::new(tx) }
    }

    fn send(&self, cmd: PlayerCmd) -> Result<(), String> {
        self.tx
            .lock()
            .map_err(|_| "player: lock envenenado".to_string())?
            .send(cmd)
            .map_err(|_| "player: thread do VLC encerrou".to_string())
    }

    pub async fn play(&self, path: String, start_at_seconds: f64) -> Result<(), String> {
        let (reply, rx) = oneshot::channel();
        self.send(PlayerCmd::Play { path, start_at_seconds, reply })?;
        rx.await.map_err(|_| "player: sem resposta".to_string())?
    }

    pub fn set_pause(&self, paused: bool) -> Result<(), String> {
        self.send(PlayerCmd::SetPause(paused))
    }

    pub fn toggle_pause(&self) -> Result<(), String> {
        self.send(PlayerCmd::TogglePause)
    }

    pub fn seek(&self, seconds: f64) -> Result<(), String> {
        self.send(PlayerCmd::Seek(seconds))
    }

    pub fn set_volume(&self, volume: i32) -> Result<(), String> {
        self.send(PlayerCmd::SetVolume(volume))
    }

    pub fn stop(&self) -> Result<(), String> {
        self.send(PlayerCmd::Stop)
    }

    pub async fn get_position(&self) -> Option<f64> {
        let (reply, rx) = oneshot::channel();
        self.send(PlayerCmd::GetPosition(reply)).ok()?;
        rx.await.ok().flatten()
    }

    pub async fn get_duration(&self) -> Option<f64> {
        let (reply, rx) = oneshot::channel();
        self.send(PlayerCmd::GetDuration(reply)).ok()?;
        rx.await.ok().flatten()
    }

    pub async fn has_ended(&self) -> bool {
        let (reply, rx) = oneshot::channel();
        if self.send(PlayerCmd::HasEnded(reply)).is_err() {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    pub fn set_rate(&self, rate: f32) -> Result<(), String> {
        self.send(PlayerCmd::SetRate(rate))
    }

    pub fn set_audio_delay(&self, ms: i64) -> Result<(), String> {
        self.send(PlayerCmd::SetAudioDelay(ms))
    }

    pub fn set_subtitle_delay(&self, ms: i64) -> Result<(), String> {
        self.send(PlayerCmd::SetSubtitleDelay(ms))
    }

    pub fn add_subtitle_file(&self, path: &str) -> Result<(), String> {
        self.send(PlayerCmd::AddSubtitleFile(path.to_string()))
    }

    /// Estado do áudio para diagnóstico (autoteste).
    pub async fn audio_debug(&self) -> String {
        let (reply, rx) = oneshot::channel();
        if self.send(PlayerCmd::AudioDebug(reply)).is_err() {
            return "thread do player indisponível".to_string();
        }
        rx.await.unwrap_or_else(|_| "sem resposta".to_string())
    }

    pub async fn get_tracks(&self) -> Result<TrackMenu, String> {
        let (reply, rx) = oneshot::channel();
        self.send(PlayerCmd::GetTracks(reply))?;
        rx.await.map_err(|_| "player: sem resposta".to_string())
    }

    pub fn set_audio_track(&self, id: i32) -> Result<(), String> {
        self.send(PlayerCmd::SetAudioTrack(id))
    }

    pub fn set_subtitle_track(&self, id: i32) -> Result<(), String> {
        self.send(PlayerCmd::SetSubtitleTrack(id))
    }
}
