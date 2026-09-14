//! Comandos de leitura da biblioteca, settings e intro markers (spec §9–§10).

use sqlx::PgPool;
use tauri::State;

use crate::commands::player::SettingsCache;
use crate::db::models::{
    CollectionCard, CollectionDetail, CollectionType, ContinueWatchingItem, IntroMarkerDto,
    LibraryRoot, SeasonWithVideos, UserSettings, VideoRow,
};
use crate::db::repo;
use crate::scan::natural_sort;

#[tauri::command]
pub async fn get_collections(pool: State<'_, PgPool>) -> Result<Vec<CollectionCard>, String> {
    let mut cards = repo::fetch_collections(&pool).await.map_err(|e| e.to_string())?;
    cards.sort_by(|a, b| natural_sort::compare(&a.title, &b.title));
    Ok(cards)
}

#[tauri::command]
pub async fn get_collection_detail(
    collection_id: i32,
    pool: State<'_, PgPool>,
) -> Result<CollectionDetail, String> {
    let (title, type_str) = repo::fetch_collection_title_type(&pool, collection_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Coleção {collection_id} não encontrada"))?;
    let collection_type = CollectionType::from_db_str(&type_str)?;
    let show_thumbnails = repo::fetch_collection_thumbnail_visibility(&pool, collection_id)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or(true);

    let mut seasons = repo::fetch_seasons(&pool, collection_id).await.map_err(|e| e.to_string())?;
    seasons.sort_by(|a, b| natural_sort::compare(&a.title, &b.title));
    let mut videos = repo::fetch_collection_videos(&pool, collection_id)
        .await
        .map_err(|e| e.to_string())?;
    videos.sort_by(|a, b| natural_sort::compare(&a.display_name, &b.display_name));

    let mut loose_videos = Vec::new();
    let mut season_groups: Vec<SeasonWithVideos> = seasons
        .into_iter()
        .map(|season| SeasonWithVideos { season, videos: Vec::new() })
        .collect();
    for video in videos {
        match video.season_id {
            Some(season_id) => {
                if let Some(group) = season_groups.iter_mut().find(|g| g.season.id == season_id) {
                    group.videos.push(video);
                }
            }
            None => loose_videos.push(video),
        }
    }

    Ok(CollectionDetail {
        id: collection_id,
        title,
        collection_type,
        show_thumbnails,
        seasons: season_groups,
        loose_videos,
    })
}

#[tauri::command]
pub async fn get_video(video_id: i32, pool: State<'_, PgPool>) -> Result<Option<VideoRow>, String> {
    repo::fetch_video(&pool, video_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_all_video_ids_in_collection(
    collection_id: i32,
    pool: State<'_, PgPool>,
) -> Result<Vec<i32>, String> {
    repo::fetch_all_video_ids_in_collection(&pool, collection_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_continue_watching(pool: State<'_, PgPool>) -> Result<Vec<ContinueWatchingItem>, String> {
    repo::fetch_continue_watching(&pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_library_roots(pool: State<'_, PgPool>) -> Result<Vec<LibraryRoot>, String> {
    repo::fetch_library_roots(&pool).await.map_err(|e| e.to_string())
}

/// Marcar assistido à mão; desmarcar zera o progresso (rever do início).
#[tauri::command]
pub async fn set_video_watched(
    video_id: i32,
    watched: bool,
    pool: State<'_, PgPool>,
) -> Result<(), String> {
    repo::set_video_watched(&pool, video_id, watched)
        .await
        .map_err(|e| e.to_string())
}

/// Remove uma pasta da biblioteca (os arquivos no disco não são tocados).
#[tauri::command]
pub async fn remove_library_root(root_id: i32, pool: State<'_, PgPool>) -> Result<(), String> {
    repo::delete_library_root(&pool, root_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(pool: State<'_, PgPool>) -> Result<UserSettings, String> {
    repo::fetch_settings(&pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_auto_skip_intro(
    enabled: bool,
    pool: State<'_, PgPool>,
    cache: State<'_, SettingsCache>,
) -> Result<(), String> {
    repo::update_auto_skip_intro(&pool, enabled).await.map_err(|e| e.to_string())?;
    cache.set_auto_skip(enabled);
    Ok(())
}

#[tauri::command]
pub async fn get_intro_marker(
    video_id: i32,
    pool: State<'_, PgPool>,
) -> Result<Option<IntroMarkerDto>, String> {
    repo::fetch_intro_marker_for_video(&pool, video_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_intro_marker(
    collection_id: i32,
    season_id: Option<i32>,
    start_seconds: f64,
    end_seconds: f64,
    pool: State<'_, PgPool>,
) -> Result<(), String> {
    if end_seconds <= start_seconds {
        return Err("Fim da abertura deve ser maior que o início".to_string());
    }
    repo::upsert_intro_marker(&pool, collection_id, season_id, start_seconds, end_seconds)
        .await
        .map_err(|e| e.to_string())
}

