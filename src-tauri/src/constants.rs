//! Ponto único de definição de literais do backend (spec §4.1).
#![allow(dead_code)] // constantes das fases futuras ainda sem uso

/// Nome da variável de ambiente com a string de conexão do Postgres.
pub const ENV_DATABASE_URL: &str = "DATABASE_URL";

/// Arquivo de configuração procurado ao lado do executável instalado.
pub const ENV_FILE_NAME: &str = ".env";

/// Título das caixas de erro fatal (app sem console no release).
pub const FATAL_ERROR_TITLE: &str = "player_moovia";

/// Variável de ambiente opcional com o diretório da instalação do VLC.
pub const ENV_VLC_DIR: &str = "VLC_DIR";

/// Instalação padrão do VLC 64-bit no Windows.
pub const DEFAULT_VLC_DIR: &str = r"C:\Program Files\VideoLAN\VLC";

/// Extensões de arquivo de vídeo aceitas pelo scanner (sem o ponto, minúsculas).
pub const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "mov", "wmv", "m4v", "webm"];

/// Fração da duração a partir da qual o vídeo é marcado como assistido.
pub const WATCHED_THRESHOLD_RATIO: f64 = 0.9;

/// Intervalo entre salvamentos periódicos de progresso de reprodução.
pub const PROGRESS_SAVE_INTERVAL_SECS: u64 = 5;

/// Intervalo do tick de acompanhamento da reprodução (posição, evento, auto-skip).
pub const PLAYER_TICK_MS: u64 = 1000;

/// Id de trilha que representa "desativado" no libVLC.
pub const TRACK_DISABLED_ID: i32 = -1;

/// Saída de áudio nula do libVLC — usada pela prévia para não encostar no
/// dispositivo de som (a sessão de áudio é compartilhada pelo processo).
pub const AUDIO_OUTPUT_DUMMY: &str = "adummy";

/// Tentativas de reaplicar a trilha preferida enquanto o libVLC ainda não
/// terminou de listar as trilhas do arquivo recém-aberto.
pub const TRACK_APPLY_MAX_ATTEMPTS: u32 = 15;

/// Dimensões do frame de prévia da barra de progresso.
pub const PREVIEW_WIDTH: u32 = 256;
pub const PREVIEW_HEIGHT: u32 = 144;

/// Chroma pedido ao libVLC para a prévia (BGRA em little-endian).
pub const PREVIEW_CHROMA: &str = "RV32";

/// Tempo máximo esperando o frame após o seek da prévia.
pub const PREVIEW_FRAME_TIMEOUT_MS: u64 = 2500;

/// Intervalo de polling enquanto o frame da prévia não chega.
pub const PREVIEW_POLL_MS: u64 = 15;

/// Dimensões da capa gerada a partir de um frame do primeiro episódio.
pub const POSTER_WIDTH: u32 = 640;
pub const POSTER_HEIGHT: u32 = 360;

/// Instante padrão sugerido para capturar a capa (evita logo/abertura).
pub const POSTER_DEFAULT_SECONDS: f64 = 300.0;

/// Subpasta (no app data) onde as capas geradas são gravadas.
pub const POSTERS_SUBDIR: &str = "posters";

// ---- Renomeação em lote ----

/// Marcadores aceitos no padrão de nome dos arquivos.
pub const TOKEN_SERIES: &str = "{serie}";
pub const TOKEN_SEASON: &str = "{temporada}";
pub const TOKEN_EPISODE: &str = "{episodio}";
pub const TOKEN_TITLE: &str = "{titulo}";
/// Nome do episódio já separado das tags de release.
pub const TOKEN_EPISODE_NAME: &str = "{nome}";

/// Padrão sugerido na UI.
pub const DEFAULT_RENAME_PATTERN: &str = "S{temporada}E{episodio} - {nome}";

/// Casas usadas ao formatar temporada/episódio (S01E02).
pub const RENAME_NUMBER_PAD: usize = 2;

/// Caracteres proibidos em nome de arquivo no Windows.
pub const INVALID_FILENAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// Substituto para caracteres proibidos.
pub const INVALID_CHAR_REPLACEMENT: &str = "-";

/// Máximo de conexões do pool Postgres.
pub const DB_MAX_CONNECTIONS: u32 = 5;

/// Máximo de itens na seção "Continue assistindo".
pub const CONTINUE_WATCHING_LIMIT: i64 = 20;

/// Liga o autoteste de áudio no startup (diagnóstico — ver commands/player.rs).
pub const ENV_SELFTEST: &str = "MOOVIA_SELFTEST";

/// Label da janela principal (tauri.conf.json).
pub const MAIN_WINDOW_LABEL: &str = "main";

/// Label da janela sobreposta com os controles do player.
pub const OVERLAY_WINDOW_LABEL: &str = "player-overlay";

/// Rota do frontend carregada na janela sobreposta.
pub const OVERLAY_URL_BASE: &str = "index.html?overlay=1";

/// Evento emitido quando o player é fechado (a biblioteca recarrega o progresso).
pub const EVENT_PLAYER_CLOSED: &str = "player-closed";

/// Evento emitido a cada tick de reprodução com posição/duração.
pub const EVENT_PLAYBACK_PROGRESS: &str = "playback-progress";

/// Evento emitido quando a reprodução termina ou é interrompida.
pub const EVENT_PLAYBACK_STOPPED: &str = "playback-stopped";

/// Evento emitido quando o vídeo chega ao fim (dispara o próximo episódio).
pub const EVENT_PLAYBACK_ENDED: &str = "playback-ended";

/// Progresso da geração de miniaturas em lote.
pub const EVENT_THUMBNAIL_PROGRESS: &str = "thumbnail-progress";

/// Progresso da análise local de introdução.
pub const EVENT_INTRO_ANALYSIS_PROGRESS: &str = "intro-analysis-progress";
pub const INTRO_ANALYSIS_MAX_SECONDS: f64 = 300.0;
pub const INTRO_ANALYSIS_MIN_INTERVAL_SECONDS: f64 = 2.0;
pub const INTRO_ANALYSIS_MAX_EPISODES: usize = 30;
pub const INTRO_ANALYSIS_FRAME_WIDTH: u32 = 96;
pub const INTRO_ANALYSIS_FRAME_HEIGHT: u32 = 54;
pub const INTRO_ANALYSIS_HASH_DISTANCE: u32 = 12;
pub const INTRO_ANALYSIS_MIN_MATCH_RATIO: f64 = 0.6;

/// Extensões de legenda externa procuradas ao lado do vídeo.
pub const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "sub", "vtt"];

/// Miniaturas dos episódios: subpasta no app data e instante da captura.
pub const THUMBS_SUBDIR: &str = "thumbs";
pub const THUMB_WIDTH: u32 = 320;
pub const THUMB_HEIGHT: u32 = 180;
/// Fração da duração usada para a miniatura (evita logo de abertura).
pub const THUMB_POSITION_RATIO: f64 = 0.35;
/// Instante usado quando a duração ainda não é conhecida.
pub const THUMB_FALLBACK_SECONDS: f64 = 300.0;
