mod commands;
mod config;
mod constants;
mod db;
mod scan;
pub mod vlc_engine;

use tauri::Manager;

use std::sync::Arc;

use commands::player::{PlaybackState, SettingsCache, TrackPrefs};
use commands::organize::ThumbnailJob;
use commands::intro::IntroAnalysisJob;
use config::AppConfig;
use constants::{ENV_SELFTEST, FATAL_ERROR_TITLE, MAIN_WINDOW_LABEL};
use vlc_engine::engine::PlayerHandle;
use vlc_engine::host::{self, VideoHost};
use vlc_engine::preview::PreviewHandle;

#[link(name = "kernel32")]
extern "system" {
    fn SetDllDirectoryW(path: *const u16) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(hwnd: *mut std::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

const MB_ICONERROR: u32 = 0x0000_0010;

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// No release não há console: sem isto, uma falha de configuração ou de banco
/// faria o app fechar sem explicação nenhuma.
fn fatal(message: &str) -> ! {
    eprintln!("{message}");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            to_wide(message).as_ptr(),
            to_wide(FATAL_ERROR_TITLE).as_ptr(),
            MB_ICONERROR,
        );
    }
    std::process::exit(1);
}

/// A libvlc.dll é delay-loaded (build.rs); este diretório precisa estar no search
/// path ANTES da primeira chamada ao libVLC para que a DLL venha da instalação do
/// VLC e os plugins sejam descobertos relativo à libvlccore.dll.
fn register_vlc_dll_dir(vlc_dir: &str) {
    let wide: Vec<u16> = vlc_dir.encode_utf16().chain(std::iter::once(0)).collect();
    let ok = unsafe { SetDllDirectoryW(wide.as_ptr()) };
    if ok == 0 {
        eprintln!("SetDllDirectoryW falhou para {vlc_dir}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    config::load_env();
    let app_config = match AppConfig::from_env() {
        Ok(config) => config,
        Err(e) => fatal(&e),
    };
    register_vlc_dll_dir(&app_config.vlc_dir);

    let pool = match tauri::async_runtime::block_on(db::init_pool(&app_config)) {
        Ok(pool) => pool,
        Err(e) => fatal(&format!(
            "Não foi possível conectar ao banco player_moovia.\n\nVerifique se o PostgreSQL está \
             rodando e se a DATABASE_URL está correta.\n\nDetalhe: {e}"
        )),
    };
    let settings = match tauri::async_runtime::block_on(db::repo::fetch_settings(&pool)) {
        Ok(settings) => settings,
        Err(e) => fatal(&format!("Falha ao ler as configurações no banco.\n\nDetalhe: {e}")),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_config)
        .manage(pool)
        .manage(SettingsCache::new(settings.auto_skip_intro))
        .manage(PlaybackState::default())
        .manage(TrackPrefs::default())
        .manage(ThumbnailJob::default())
        .manage(IntroAnalysisJob::default())
        .manage(Arc::new(PreviewHandle::spawn()))
        .setup(|app| {
            let window = app
                .get_webview_window(MAIN_WINDOW_LABEL)
                .expect("janela principal não encontrada");
            let hwnd = window.hwnd()?.0 as isize;
            // Janela filha própria para o vídeo, no fundo do z-order — a webview
            // transparente fica por cima com os controles (ver vlc_engine/host.rs).
            let video_host = host::create_video_host(hwnd);
            app.manage(VideoHost(video_host));
            app.manage(PlayerHandle::spawn(video_host));

            if std::env::var(ENV_SELFTEST).is_ok() {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(commands::player::run_audio_selftest(handle));
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }
            match event {
                tauri::WindowEvent::CloseRequested { .. } => {
                    commands::player::finalize_on_exit(window.app_handle());
                }
                // Vídeo e controles são janelas nativas separadas: precisam
                // acompanhar a janela principal manualmente.
                tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) => {
                    commands::player::sync_player_surfaces(window.app_handle());
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::scanner::scan_library,
            commands::scanner::rescan_all,
            commands::library::get_collections,
            commands::library::get_collection_detail,
            commands::library::get_video,
            commands::library::get_continue_watching,
            commands::library::get_library_roots,
            commands::library::set_video_watched,
            commands::library::remove_library_root,
            commands::library::get_settings,
            commands::library::set_auto_skip_intro,
            commands::library::get_intro_marker,
            commands::library::set_intro_marker,
            commands::intro::analyze_intro,
            commands::intro::cancel_intro_analysis,
            commands::intro::accept_intro_suggestion,
            commands::player::play_video,
            commands::player::toggle_pause,
            commands::player::seek,
            commands::player::get_current_position,
            commands::player::get_duration,
            commands::player::set_volume,
            commands::player::get_tracks,
            commands::player::set_audio_track,
            commands::player::set_subtitle_track,
            commands::player::get_preview_frame,
            commands::player::set_playback_rate,
            commands::player::set_audio_delay,
            commands::player::set_subtitle_delay,
            commands::player::stop_playback,
            commands::removal::remove_video_from_library,
            commands::removal::delete_video_file,
            commands::removal::remove_collection_from_library,
            commands::removal::delete_collection_files,
            commands::organize::preview_episode_names,
            commands::organize::apply_episode_names,
            commands::organize::set_video_display_name,
            commands::organize::preview_rename,
            commands::organize::apply_rename,
            commands::organize::set_collection_title,
            commands::organize::set_collection_type,
            commands::organize::merge_collection_into,
            commands::organize::generate_collection_poster,
            commands::organize::generate_episode_thumbnails,
            commands::organize::cancel_episode_thumbnails,
            commands::organize::generate_episode_thumbnail,
            commands::organize::set_collection_thumbnails_visible,
            commands::organize::get_episode_thumbnail,
            commands::organize::get_collection_poster,
            commands::player::open_player,
            commands::player::close_player,
            commands::player::set_player_fullscreen,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
