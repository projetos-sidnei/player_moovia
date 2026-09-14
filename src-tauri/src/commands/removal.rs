//! Remoção de vídeos e coleções, em dois níveis:
//! - "do player": apaga só os registros; os arquivos continuam no disco.
//! - "do PC": manda os arquivos para a Lixeira (recuperável) e apaga os registros.
//!
//! Toda chamada aqui é destrutiva — a UI sempre pede confirmação antes.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sqlx::PgPool;
use tauri::{AppHandle, Manager};

use crate::commands::player::stop_if_playing;
use crate::db::repo;
use crate::scan::natural_sort;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    /// Arquivos enviados para a Lixeira.
    pub trashed: usize,
    pub errors: Vec<String>,
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Manda o arquivo para a Lixeira do Windows (recuperável).
fn send_to_trash(path: &str) -> Result<(), String> {
    if !Path::new(path).exists() {
        return Ok(()); // já não está lá — segue para remover o registro
    }
    trash::delete(path).map_err(|e| format!("{}: {e}", file_name(path)))
}

/// Remove pastas que ficaram vazias, da mais profunda para a mais rasa.
fn remove_empty_dirs(mut dirs: Vec<PathBuf>) {
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    dirs.dedup();
    for dir in dirs {
        // Falha (silenciosa) se ainda houver qualquer coisa dentro.
        let _ = std::fs::remove_dir(&dir);
    }
}

// ---------- Vídeo ----------

/// Tira o episódio da biblioteca; o arquivo permanece no disco.
#[tauri::command]
pub async fn remove_video_from_library(video_id: i32, app: AppHandle) -> Result<(), String> {
    stop_if_playing(&app, &[video_id]).await;
    let pool = app.state::<PgPool>();
    repo::delete_video(&pool, video_id).await.map_err(|e| e.to_string())
}

/// Manda o arquivo do episódio para a Lixeira e tira da biblioteca.
#[tauri::command]
pub async fn delete_video_file(video_id: i32, app: AppHandle) -> Result<DeleteOutcome, String> {
    stop_if_playing(&app, &[video_id]).await;
    let pool = app.state::<PgPool>();
    let video = repo::fetch_video(&pool, video_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Vídeo {video_id} não encontrado"))?;

    let mut outcome = DeleteOutcome::default();
    match send_to_trash(&video.file_path) {
        Ok(()) => {
            outcome.trashed += 1;
            repo::delete_video(&pool, video_id).await.map_err(|e| e.to_string())?;
        }
        Err(e) => outcome.errors.push(e),
    }
    Ok(outcome)
}

// ---------- Coleção ----------

/// Tira a coleção inteira da biblioteca; os arquivos permanecem no disco.
/// (um novo scan da raiz traz tudo de volta)
#[tauri::command]
pub async fn remove_collection_from_library(collection_id: i32, app: AppHandle) -> Result<(), String> {
    let pool = app.state::<PgPool>();
    let videos = repo::fetch_collection_videos(&pool, collection_id)
        .await
        .map_err(|e| e.to_string())?;
    let ids: Vec<i32> = videos.iter().map(|v| v.id).collect();
    stop_if_playing(&app, &ids).await;
    repo::delete_collection(&pool, collection_id).await.map_err(|e| e.to_string())
}

/// Manda todos os vídeos da coleção para a Lixeira e tira da biblioteca.
/// Pastas que sobrarem vazias são removidas; pastas com outros arquivos ficam.
#[tauri::command]
pub async fn delete_collection_files(collection_id: i32, app: AppHandle) -> Result<DeleteOutcome, String> {
    let pool = app.state::<PgPool>();
    let mut videos = repo::fetch_collection_videos(&pool, collection_id)
        .await
        .map_err(|e| e.to_string())?;
    videos.sort_by(|a, b| natural_sort::compare(&a.file_path, &b.file_path));

    let ids: Vec<i32> = videos.iter().map(|v| v.id).collect();
    stop_if_playing(&app, &ids).await;

    let mut outcome = DeleteOutcome::default();
    let mut dirs: Vec<PathBuf> = Vec::new();
    for video in &videos {
        match send_to_trash(&video.file_path) {
            Ok(()) => {
                outcome.trashed += 1;
                if let Some(parent) = Path::new(&video.file_path).parent() {
                    dirs.push(parent.to_path_buf());
                }
            }
            Err(e) => outcome.errors.push(e),
        }
    }

    // Só limpa os registros se tudo saiu do disco; senão a biblioteca ficaria
    // fora de sincronia com o que sobrou.
    if outcome.errors.is_empty() {
        if let Some(folder) = repo::fetch_collection_folder(&pool, collection_id)
            .await
            .map_err(|e| e.to_string())?
        {
            dirs.push(PathBuf::from(folder));
        }
        repo::delete_collection(&pool, collection_id).await.map_err(|e| e.to_string())?;
        remove_empty_dirs(dirs);
    }
    Ok(outcome)
}
