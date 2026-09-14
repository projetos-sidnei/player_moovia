//! Comandos de reprodução + monitor de progresso (spec §8–§9).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use sqlx::PgPool;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::constants::{
    EVENT_PLAYBACK_ENDED, EVENT_PLAYBACK_PROGRESS, EVENT_PLAYBACK_STOPPED, EVENT_PLAYER_CLOSED,
    MAIN_WINDOW_LABEL, OVERLAY_URL_BASE, OVERLAY_WINDOW_LABEL, PLAYER_TICK_MS,
    PROGRESS_SAVE_INTERVAL_SECS, SUBTITLE_EXTENSIONS, TRACK_APPLY_MAX_ATTEMPTS, TRACK_DISABLED_ID,
};
use crate::db::models::IntroMarkerDto;
use crate::db::repo;
use crate::vlc_engine::engine::{PlayerHandle, TrackInfo, TrackMenu};
use crate::vlc_engine::host::{self, VideoHost};
use crate::vlc_engine::preview::PreviewHandle;

/// Trilha escolhida pelo usuário. Os ids do libVLC são por arquivo, então a
/// preferência é guardada pelo NOME da trilha ("Português", "English"...) e
/// reaplicada por nome no próximo vídeo.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackChoice {
    Disabled,
    Named(String),
}

impl TrackChoice {
    /// Só vira preferência o que dá para reencontrar por nome no próximo arquivo.
    /// Trilha desconhecida ou sem nome → None (esquece a preferência) em vez de
    /// virar `Disabled`, que desligaria a trilha nos vídeos seguintes.
    fn from_selection(track_id: i32, tracks: &[TrackInfo]) -> Option<Self> {
        if track_id == TRACK_DISABLED_ID {
            return Some(TrackChoice::Disabled);
        }
        tracks
            .iter()
            .find(|t| t.id == track_id)
            .filter(|t| !t.name.is_empty())
            .map(|t| TrackChoice::Named(t.name.clone()))
    }

    /// Id equivalente no arquivo atual, ou None se aquela trilha não existe aqui.
    fn resolve(&self, tracks: &[TrackInfo]) -> Option<i32> {
        match self {
            TrackChoice::Disabled => Some(TRACK_DISABLED_ID),
            TrackChoice::Named(name) => tracks
                .iter()
                .find(|t| t.name == *name)
                .or_else(|| tracks.iter().find(|t| t.name.eq_ignore_ascii_case(name)))
                .map(|t| t.id),
        }
    }
}

/// Preferências de trilha da sessão (sobrevivem à troca de episódio).
#[derive(Default)]
pub struct TrackPrefs {
    audio: Mutex<Option<TrackChoice>>,
    subtitle: Mutex<Option<TrackChoice>>,
}

impl TrackPrefs {
    fn remember(slot: &Mutex<Option<TrackChoice>>, choice: Option<TrackChoice>) {
        if let Ok(mut guard) = slot.lock() {
            *guard = choice;
        }
    }

    fn read(slot: &Mutex<Option<TrackChoice>>) -> Option<TrackChoice> {
        slot.lock().ok().and_then(|guard| guard.clone())
    }

    /// Áudio NUNCA guarda "desativado": um vídeo mudo não é preferência que se
    /// queira arrastar para os próximos episódios.
    pub fn remember_audio(&self, track_id: i32, tracks: &[TrackInfo]) {
        let choice = TrackChoice::from_selection(track_id, tracks)
            .filter(|choice| !matches!(choice, TrackChoice::Disabled));
        Self::remember(&self.audio, choice);
    }

    /// Legenda desligada é uma preferência legítima e persiste.
    pub fn remember_subtitle(&self, track_id: i32, tracks: &[TrackInfo]) {
        Self::remember(&self.subtitle, TrackChoice::from_selection(track_id, tracks));
    }

    /// Reaplica as escolhas do usuário no vídeo recém-aberto.
    fn apply(&self, player: &PlayerHandle, menu: &TrackMenu) {
        if let Some(choice) = Self::read(&self.audio) {
            // Sem correspondência por nome, deixa o padrão do arquivo tocar.
            if let Some(id) = choice.resolve(&menu.audio) {
                let _ = player.set_audio_track(id);
            }
        }
        if let Some(choice) = Self::read(&self.subtitle) {
            if let Some(id) = choice.resolve(&menu.subtitles) {
                let _ = player.set_subtitle_track(id);
            }
        }
    }
}

/// Cache em memória do `user_settings.auto_skip_intro` (evita query por tick).
pub struct SettingsCache {
    auto_skip_intro: AtomicBool,
}

impl SettingsCache {
    pub fn new(auto_skip_intro: bool) -> Self {
        Self { auto_skip_intro: AtomicBool::new(auto_skip_intro) }
    }
    pub fn auto_skip(&self) -> bool {
        self.auto_skip_intro.load(Ordering::Relaxed)
    }
    pub fn set_auto_skip(&self, enabled: bool) {
        self.auto_skip_intro.store(enabled, Ordering::Relaxed);
    }
}

pub struct PlaybackSession {
    pub video_id: i32,
    pub monitor: tauri::async_runtime::JoinHandle<()>,
}

/// Sessão de reprodução atual (no máximo uma).
pub struct PlaybackState(pub tokio::sync::Mutex<Option<PlaybackSession>>);

impl Default for PlaybackState {
    fn default() -> Self {
        Self(tokio::sync::Mutex::new(None))
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStartInfo {
    pub video_id: i32,
    pub start_at_seconds: f64,
    pub duration_seconds: Option<f64>,
    pub intro_marker: Option<IntroMarkerDto>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    video_id: i32,
    position_seconds: f64,
    duration_seconds: Option<f64>,
}

/// Salva a posição atual do vídeo em reprodução (salvamento final forçado, spec §8).
async fn save_now(pool: &PgPool, player: &PlayerHandle, video_id: i32) {
    if let Some(pos) = player.get_position().await {
        let duration = repo::fetch_video(pool, video_id)
            .await
            .ok()
            .flatten()
            .and_then(|v| v.duration_seconds)
            .or(player.get_duration().await);
        let _ = repo::save_progress(pool, video_id, pos, duration).await;
    }
}

/// Encerra a sessão atual: aborta o monitor e força o salvamento final.
async fn finalize_session(app: &AppHandle) {
    let playback = app.state::<PlaybackState>();
    let session = playback.0.lock().await.take();
    if let Some(session) = session {
        session.monitor.abort();
        let pool = app.state::<PgPool>();
        let player = app.state::<PlayerHandle>();
        save_now(&pool, &player, session.video_id).await;
    }
}

/// Chamado no fechamento da janela: salvamento final síncrono.
pub fn finalize_on_exit(app: &AppHandle) {
    tauri::async_runtime::block_on(finalize_session(app));
}

/// Para a reprodução se um dos vídeos estiver tocando — o Windows não deixa
/// apagar/mover arquivo aberto pelo VLC.
pub async fn stop_if_playing(app: &AppHandle, video_ids: &[i32]) {
    let playing = {
        let playback = app.state::<PlaybackState>();
        let guard = playback.0.lock().await;
        guard.as_ref().map(|s| s.video_id)
    };
    if playing.is_some_and(|id| video_ids.contains(&id)) {
        finalize_session(app).await;
        let player = app.state::<PlayerHandle>();
        let _ = player.stop();
        app.state::<Arc<PreviewHandle>>().close();
    }
}

fn spawn_monitor(
    app: AppHandle,
    video_id: i32,
    mut duration: Option<f64>,
    marker: Option<IntroMarkerDto>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let pool = app.state::<PgPool>();
        let player = app.state::<PlayerHandle>();
        let settings = app.state::<SettingsCache>();
        let prefs = app.state::<TrackPrefs>();
        let mut tick: u64 = 0;
        let mut last_position: Option<f64> = None;
        let mut tracks_applied = false;
        let mut track_attempts: u32 = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(PLAYER_TICK_MS)).await;

            // As trilhas só ficam listáveis alguns instantes depois do play —
            // tenta reaplicar a escolha do usuário até elas aparecerem.
            if !tracks_applied && track_attempts < TRACK_APPLY_MAX_ATTEMPTS {
                track_attempts += 1;
                if let Ok(menu) = player.get_tracks().await {
                    if !menu.audio.is_empty() || !menu.subtitles.is_empty() {
                        prefs.apply(&player, &menu);
                        tracks_applied = true;
                    }
                }
            }

            // Fim do vídeo: salva como assistido e avisa a UI para emendar o
            // próximo episódio. O monitor encerra aqui.
            if player.has_ended().await {
                let final_position = duration.unwrap_or_else(|| last_position.unwrap_or(0.0));
                let _ = repo::save_progress(&pool, video_id, final_position, duration).await;
                let _ = app.emit(EVENT_PLAYBACK_ENDED, video_id);
                return;
            }

            let Some(mut position) = player.get_position().await else { continue };

            // Pausado: posição parada → tick sem trabalho (sem evento, sem
            // query, sem UPSERT).
            if last_position.map(|p| (p - position).abs() < 0.01).unwrap_or(false) {
                continue;
            }
            last_position = Some(position);
            tick += 1;

            if duration.is_none() {
                if let Some(d) = player.get_duration().await {
                    let _ = repo::set_video_duration(&pool, video_id, d).await;
                    duration = Some(d);
                }
            }

            if let Some(m) = marker {
                if settings.auto_skip() && position >= m.start_seconds && position < m.end_seconds {
                    let _ = player.seek(m.end_seconds);
                    position = m.end_seconds;
                }
            }

            let _ = app.emit(
                EVENT_PLAYBACK_PROGRESS,
                ProgressEvent { video_id, position_seconds: position, duration_seconds: duration },
            );

            if tick % PROGRESS_SAVE_INTERVAL_SECS == 0 {
                let _ = repo::save_progress(&pool, video_id, position, duration).await;
            }
        }
    })
}

#[tauri::command]
pub async fn play_video(video_id: i32, app: AppHandle) -> Result<PlaybackStartInfo, String> {
    finalize_session(&app).await;

    let pool = app.state::<PgPool>();
    let video = repo::fetch_video(&pool, video_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Vídeo {video_id} não encontrado"))?;
    if !std::path::Path::new(&video.file_path).is_file() {
        return Err(format!("Arquivo não encontrado no disco: {}", video.file_path));
    }

    let mut start_at = repo::fetch_resume_position(&pool, video_id).await.map_err(|e| e.to_string())?;
    // Rever um episódio já assistido recomeça do zero em vez de retomar no fim.
    if let Some(d) = video.duration_seconds {
        if start_at >= d - 1.0 {
            start_at = 0.0;
        }
    }
    let marker = repo::fetch_intro_marker_for_video(&pool, video_id).await.map_err(|e| e.to_string())?;

    let player = app.state::<PlayerHandle>();
    player.play(video.file_path.clone(), start_at).await?;
    app.state::<Arc<PreviewHandle>>().open(&video.file_path);

    // Legendas externas na mesma pasta (.srt e afins) entram como faixas.
    for legenda in sibling_subtitles(&video.file_path) {
        let _ = player.add_subtitle_file(&legenda);
    }

    let monitor = spawn_monitor(app.clone(), video_id, video.duration_seconds, marker);
    let playback = app.state::<PlaybackState>();
    *playback.0.lock().await = Some(PlaybackSession { video_id, monitor });

    Ok(PlaybackStartInfo {
        video_id,
        start_at_seconds: start_at,
        duration_seconds: video.duration_seconds,
        intro_marker: marker,
    })
}

/// Pausa/retoma; ao pausar força salvamento final (spec §8).
#[tauri::command]
pub async fn toggle_pause(app: AppHandle) -> Result<(), String> {
    let player = app.state::<PlayerHandle>();
    player.toggle_pause()?;
    let playback = app.state::<PlaybackState>();
    let video_id = playback.0.lock().await.as_ref().map(|s| s.video_id);
    if let Some(video_id) = video_id {
        let pool = app.state::<PgPool>();
        save_now(&pool, &player, video_id).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn seek(position_seconds: f64, app: AppHandle) -> Result<(), String> {
    let player = app.state::<PlayerHandle>();
    player.seek(position_seconds)
}

#[tauri::command]
pub async fn get_current_position(player: State<'_, PlayerHandle>) -> Result<Option<f64>, String> {
    Ok(player.get_position().await)
}

#[tauri::command]
pub async fn get_duration(player: State<'_, PlayerHandle>) -> Result<Option<f64>, String> {
    Ok(player.get_duration().await)
}

#[tauri::command]
pub async fn set_volume(volume: i32, player: State<'_, PlayerHandle>) -> Result<(), String> {
    player.set_volume(volume)
}

#[tauri::command]
pub async fn get_tracks(player: State<'_, PlayerHandle>) -> Result<TrackMenu, String> {
    player.get_tracks().await
}

#[tauri::command]
pub async fn set_audio_track(
    track_id: i32,
    player: State<'_, PlayerHandle>,
    prefs: State<'_, TrackPrefs>,
) -> Result<(), String> {
    let menu = player.get_tracks().await.unwrap_or_default();
    prefs.remember_audio(track_id, &menu.audio);
    player.set_audio_track(track_id)
}

#[tauri::command]
pub async fn set_subtitle_track(
    track_id: i32,
    player: State<'_, PlayerHandle>,
    prefs: State<'_, TrackPrefs>,
) -> Result<(), String> {
    let menu = player.get_tracks().await.unwrap_or_default();
    prefs.remember_subtitle(track_id, &menu.subtitles);
    player.set_subtitle_track(track_id)
}

/// Frame da prévia (data URI PNG) para o hover da barra de progresso.
/// Blocante por natureza (decodifica um frame) → sai do runtime async.
#[tauri::command]
pub async fn get_preview_frame(seconds: f64, app: AppHandle) -> Result<Option<String>, String> {
    let preview = app.state::<Arc<PreviewHandle>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || preview.grab(seconds))
        .await
        .map_err(|e| e.to_string())
}

/// Para a reprodução com salvamento final (troca de vídeo/saída do player, spec §8).
#[tauri::command]
pub async fn stop_playback(app: AppHandle) -> Result<(), String> {
    finalize_session(&app).await;
    let player = app.state::<PlayerHandle>();
    player.stop()?;
    let _ = app.emit(EVENT_PLAYBACK_STOPPED, ());
    Ok(())
}

/// Arquivos de legenda ao lado do vídeo cujo nome começa com o mesmo prefixo
/// ("Ep01.mkv" → "Ep01.srt", "Ep01.pt-BR.srt").
fn sibling_subtitles(video_path: &str) -> Vec<String> {
    let path = std::path::Path::new(video_path);
    let (Some(dir), Some(stem)) = (path.parent(), path.file_stem().and_then(|s| s.to_str())) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut encontradas: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let extensao_ok = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| SUBTITLE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
                .unwrap_or(false);
            let mesmo_nome = p
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.starts_with(stem))
                .unwrap_or(false);
            extensao_ok && mesmo_nome
        })
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    encontradas.sort();
    encontradas
}

#[tauri::command]
pub async fn set_playback_rate(rate: f32, player: State<'_, PlayerHandle>) -> Result<(), String> {
    player.set_rate(rate)
}

/// Sincronia: atraso em milissegundos (positivo atrasa em relação ao vídeo).
#[tauri::command]
pub async fn set_audio_delay(delay_ms: i64, player: State<'_, PlayerHandle>) -> Result<(), String> {
    player.set_audio_delay(delay_ms)
}

#[tauri::command]
pub async fn set_subtitle_delay(delay_ms: i64, player: State<'_, PlayerHandle>) -> Result<(), String> {
    player.set_subtitle_delay(delay_ms)
}

/// Autoteste de áudio (`MOOVIA_SELFTEST=1`): toca o primeiro vídeo da biblioteca
/// pelo caminho real de produção e reporta o estado do áudio. Existe porque o
/// áudio só falhava DENTRO do app — testes isolados sempre funcionavam.
pub async fn run_audio_selftest(app: AppHandle) {
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let pool = app.state::<PgPool>();
    let video = match repo::fetch_any_video(&pool).await {
        Ok(Some(video)) => video,
        _ => {
            eprintln!("[selftest] biblioteca vazia");
            app.exit(1);
            return;
        }
    };
    eprintln!("[selftest] arquivo: {}", video.file_path);

    if let Some(host) = app.try_state::<VideoHost>() {
        host::set_video_host_visible(host.0, true);
    }
    let player = app.state::<PlayerHandle>();
    match player.play(video.file_path.clone(), 0.0).await {
        Ok(()) => eprintln!("[selftest] play ok"),
        Err(e) => eprintln!("[selftest] play FALHOU: {e}"),
    }
    let _ = player.set_volume(80);

    for segundo in 1..=6 {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        eprintln!("[selftest] t={segundo}s {}", player.audio_debug().await);
    }
    eprintln!("[selftest] fim");
    app.exit(0);
}

// ---------- Janela de vídeo + overlay de controles ----------

/// Cola a janela de controles exatamente sobre o client area da janela principal.
pub fn sync_overlay_geometry(main: &WebviewWindow, overlay: &WebviewWindow) {
    if let (Ok(pos), Ok(size)) = (main.inner_position(), main.inner_size()) {
        let _ = overlay.set_position(PhysicalPosition::new(pos.x, pos.y));
        let _ = overlay.set_size(PhysicalSize::new(size.width, size.height));
    }
}

/// Reposiciona vídeo e overlay após mover/redimensionar a janela principal.
pub fn sync_player_surfaces(app: &AppHandle) {
    let Some(main) = app.get_webview_window(MAIN_WINDOW_LABEL) else { return };
    if let (Some(video), Ok(size)) = (app.try_state::<VideoHost>(), main.inner_size()) {
        host::resize_video_host(video.0, size.width as i32, size.height as i32);
    }
    if let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        sync_overlay_geometry(&main, &overlay);
    }
}

/// Abre o player: mostra a janela do vídeo e cria a janela de controles por cima.
#[tauri::command]
pub async fn open_player(video_id: i32, collection_id: i32, app: AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "janela principal não encontrada".to_string())?;

    if let Some(previous) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        let _ = previous.destroy();
    }
    if let Some(video) = app.try_state::<VideoHost>() {
        host::set_video_host_visible(video.0, true);
    }

    let url = format!("{OVERLAY_URL_BASE}&videoId={video_id}&collectionId={collection_id}");
    let overlay = WebviewWindowBuilder::new(&app, OVERLAY_WINDOW_LABEL, WebviewUrl::App(url.into()))
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .visible(false)
        .parent(&main)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    sync_overlay_geometry(&main, &overlay);
    let _ = overlay.show();
    let _ = overlay.set_focus();
    Ok(())
}

/// Fecha o player: salvamento final, esconde o vídeo e destrói os controles.
#[tauri::command]
pub async fn close_player(app: AppHandle) -> Result<(), String> {
    finalize_session(&app).await;
    let player = app.state::<PlayerHandle>();
    let _ = player.stop();

    if let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        let _ = overlay.destroy();
    }
    if let Some(video) = app.try_state::<VideoHost>() {
        host::set_video_host_visible(video.0, false);
    }
    // Libera a instância libVLC da prévia ao sair do player.
    app.state::<Arc<PreviewHandle>>().close();
    if let Some(main) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = main.set_fullscreen(false);
        let _ = main.set_focus();
    }
    let _ = app.emit(EVENT_PLAYER_CLOSED, ());
    Ok(())
}

/// Fullscreen aplica-se à janela PRINCIPAL (dona do vídeo); o overlay acompanha.
#[tauri::command]
pub async fn set_player_fullscreen(enabled: bool, app: AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "janela principal não encontrada".to_string())?;
    main.set_fullscreen(enabled).map_err(|e| e.to_string())?;
    sync_player_surfaces(&app);
    if let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        let _ = overlay.set_focus();
    }
    Ok(())
}
