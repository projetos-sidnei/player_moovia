# Player libVLC + progresso + skip intro (spec §8–§9)

## Engine (vlc_engine/engine.rs + host.rs)

- `vlc-rs` (libvlc-sys) usando o VLC já instalado na máquina; carga por delay-load + `SetDllDirectoryW`.
- Vídeo EMBUTIDO (nunca janela do VLC nem `<video>` HTML), mas em **duas janelas**: janela filha nativa do vídeo acima da webview + janela Tauri sobreposta (`player-overlay`) com os controles. Motivo e detalhes: `decisoes.md` (2026-08-05, "ARQUITETURA DO PLAYER").
- **Só o overlay é transparente.** A janela principal é opaca (`tauri.conf.json` sem `transparent`) e os painéis do player são opacos, sem `backdrop-filter`.
- Comandos de janela: `open_player`, `close_player`, `set_player_fullscreen`. Evento `player-closed` avisa a biblioteca.
- Trilhas: `get_tracks`, `set_audio_track`, `set_subtitle_track` (escolha lembrada por NOME entre episódios — `TrackPrefs`).
- Prévia do hover: `get_preview_frame(seconds) -> Option<data URI PNG>` (segundo player headless, `vlc_engine/preview.rs`).

## Comandos IPC implementados (tauri::command) — nomes em ipc.constants.ts

- `play_video(video_id) -> PlaybackStartInfo` (finaliza sessão anterior, retoma posição, retorna marker)
- `toggle_pause()` (pausa/retoma + salvamento final; substitui o `pause()` da spec)
- `seek(position_seconds)` · `set_volume(volume)` · `stop_playback()`
- `get_current_position() -> Option<f64>` · `get_duration() -> Option<f64>`
- `get_intro_marker(video_id)` (prioridade season > collection) · `set_intro_marker(...)` · `set_auto_skip_intro(bool)`
- Eventos: `playback-progress` {videoId, positionSeconds, durationSeconds} a cada 1s; `playback-stopped`.
- Comandos de biblioteca em `commands/library.rs`; scan em `commands/scanner.rs` (`scan_library`, `rescan_all`).

## Persistência de progresso

- Salvar `position_seconds` a cada `PROGRESS_SAVE_INTERVAL_SECS` (5s) — UPSERT em `playback_progress`, com throttle (nunca por frame).
- Salvamento final FORÇADO em: pause, troca de vídeo, fechar app.
- Ao abrir vídeo com progresso salvo: seek automático ANTES de exibir o primeiro frame.
- `watched = true` quando posição >= 0.9 * duração (const `WATCHED_THRESHOLD_RATIO`).

## Skip intro (marcação manual — SEM detecção automática, fora de escopo)

1. Usuário marca start/end da abertura em qualquer episódio (botões no player ou campos na tela da coleção).
2. Salvo em `intro_markers` por `season_id` ou só `collection_id` (NULL = coleção inteira).
3. Na reprodução, backend informa o intervalo ao frontend.
4. Posição dentro de [start, end]:
   - `auto_skip_intro = true` → backend faz `seek(end_seconds)` sozinho.
   - `false` → frontend mostra botão flutuante "Pular abertura" → clique chama `seek(end_seconds)`.
5. Botão some quando posição > end_seconds.
6. Toggle de `auto_skip_intro` nas configurações (persistido em `user_settings`).
