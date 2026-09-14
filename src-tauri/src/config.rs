//! Configuração carregada uma única vez no startup e injetada via Tauri `State`.

use crate::constants::{DEFAULT_VLC_DIR, ENV_DATABASE_URL, ENV_FILE_NAME, ENV_VLC_DIR};

pub struct AppConfig {
    pub database_url: String,
    pub vlc_dir: String,
}

/// Procura o `.env` no diretório de trabalho (desenvolvimento) e, se não achar,
/// ao lado do executável — no app instalado o diretório de trabalho é o de onde
/// o atalho foi aberto, não o da instalação.
pub fn load_env() {
    if dotenvy::dotenv().is_ok() {
        return;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = dotenvy::from_path(dir.join(ENV_FILE_NAME));
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let database_url = std::env::var(ENV_DATABASE_URL).map_err(|_| {
            format!(
                "{ENV_DATABASE_URL} não definida.\n\nCrie um arquivo {ENV_FILE_NAME} ao lado do \
                 executável com:\n{ENV_DATABASE_URL}=postgres://usuario:senha@localhost:5432/player_moovia"
            )
        })?;
        let vlc_dir = std::env::var(ENV_VLC_DIR).unwrap_or_else(|_| DEFAULT_VLC_DIR.to_string());
        Ok(Self { database_url, vlc_dir })
    }
}
