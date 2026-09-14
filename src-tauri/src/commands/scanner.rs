//! Comandos de scan (spec §7). O walk é blocante → roda em spawn_blocking.

use std::path::PathBuf;

use sqlx::PgPool;
use tauri::State;

use crate::db::repo;
use crate::scan::walker::{self, ScanSummary, ScannedCollection};

async fn persist_scan(pool: &PgPool, root_id: i32, collections: &[ScannedCollection]) -> Result<ScanSummary, sqlx::Error> {
    let mut summary = ScanSummary::default();
    for col in collections {
        let collection_id = repo::upsert_collection(pool, root_id, col).await?;
        summary.collections += 1;
        for season in &col.seasons {
            let season_id =
                repo::upsert_season(pool, collection_id, &season.title, &season.folder_path, season.season_number)
                    .await?;
            summary.seasons += 1;
            for video in &season.videos {
                repo::upsert_video(pool, collection_id, Some(season_id), video).await?;
                summary.videos += 1;
            }
        }
        for video in &col.loose_videos {
            repo::upsert_video(pool, collection_id, None, video).await?;
            summary.videos += 1;
        }
    }
    summary.removed = repo::prune_missing(pool, root_id).await?;
    Ok(summary)
}

async fn scan_one_root(pool: &PgPool, root_path: String) -> Result<ScanSummary, String> {
    let path = PathBuf::from(&root_path);
    if !path.is_dir() {
        return Err(format!("Diretório não encontrado: {root_path}"));
    }
    let root_id = repo::upsert_root(pool, &root_path).await.map_err(|e| e.to_string())?;
    let collections = tauri::async_runtime::spawn_blocking(move || walker::walk_root(&path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Falha ao ler o diretório: {e}"))?;
    persist_scan(pool, root_id, &collections).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_library(root_path: String, pool: State<'_, PgPool>) -> Result<ScanSummary, String> {
    scan_one_root(&pool, root_path).await
}

/// Re-escaneia todas as raízes já cadastradas.
#[tauri::command]
pub async fn rescan_all(pool: State<'_, PgPool>) -> Result<ScanSummary, String> {
    let roots = repo::fetch_library_roots(&pool).await.map_err(|e| e.to_string())?;
    let mut total = ScanSummary::default();
    for root in roots {
        let s = scan_one_root(&pool, root.path).await?;
        total.collections += s.collections;
        total.seasons += s.seasons;
        total.videos += s.videos;
        total.removed += s.removed;
    }
    Ok(total)
}
