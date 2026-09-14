//! Organização manual da coleção: título, renomeação em lote e capa.
//! Nada aqui roda sozinho — toda ação é disparada explicitamente pela UI.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::constants::{
    INVALID_CHAR_REPLACEMENT, INVALID_FILENAME_CHARS, POSTERS_SUBDIR, RENAME_NUMBER_PAD,
    THUMBS_SUBDIR, THUMB_FALLBACK_SECONDS, THUMB_HEIGHT, THUMB_POSITION_RATIO, THUMB_WIDTH,
    EVENT_THUMBNAIL_PROGRESS, TOKEN_EPISODE, TOKEN_EPISODE_NAME, TOKEN_SEASON, TOKEN_SERIES,
    TOKEN_TITLE,
};
use crate::db::models::CollectionType;
use crate::db::repo;
use crate::scan::natural_sort;
use crate::scan::release_name;
use crate::vlc_engine::preview::{to_data_uri, PreviewHandle};

#[derive(Default)]
pub struct ThumbnailJob {
    cancel_requested: AtomicBool,
}

impl ThumbnailJob {
    fn request_cancel(&self) {
        self.cancel_requested.store(true, Ordering::Release);
    }

    fn reset(&self) {
        self.cancel_requested.store(false, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.cancel_requested.load(Ordering::Acquire)
    }
}

/// Uma renomeação proposta (nada é aplicado até o usuário confirmar).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RenamePlanItem {
    pub video_id: i32,
    pub current_name: String,
    pub new_name: String,
    /// Já existe outro arquivo com esse nome no destino.
    pub conflict: bool,
    /// Nome não muda — nada a fazer.
    pub unchanged: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameOutcome {
    pub renamed: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if INVALID_FILENAME_CHARS.contains(&c) {
                INVALID_CHAR_REPLACEMENT.chars().next().unwrap_or('-')
            } else {
                c
            }
        })
        .collect();
    cleaned.trim().trim_end_matches('.').to_string()
}

fn pad(number: i32) -> String {
    format!("{:0width$}", number.max(0), width = RENAME_NUMBER_PAD)
}

fn render_pattern(
    pattern: &str,
    series: &str,
    season: Option<i32>,
    episode: i32,
    original_title: &str,
    episode_name: &str,
) -> String {
    pattern
        .replace(TOKEN_SERIES, series)
        .replace(TOKEN_SEASON, &pad(season.unwrap_or(1)))
        .replace(TOKEN_EPISODE, &pad(episode))
        .replace(TOKEN_TITLE, original_title)
        .replace(TOKEN_EPISODE_NAME, episode_name)
        // "S01E01 - " sem nome do episódio não deve deixar sujeira no fim
        .trim()
        .trim_end_matches(['-', '–', '_'])
        .trim()
        .to_string()
}

/// Nome do episódio para o marcador `{nome}`: o definido pelo usuário vence;
/// senão tenta separar do nome do arquivo; senão fica vazio.
fn episode_name_for(video: &crate::db::models::VideoWithProgress) -> String {
    if video.display_name_locked {
        return video.display_name.clone();
    }
    release_name::parse_episode_title(&file_stem(&video.file_path)).unwrap_or_default()
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

fn target_path(current: &str, new_stem: &str) -> PathBuf {
    let path = Path::new(current);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
    let file = if extension.is_empty() {
        new_stem.to_string()
    } else {
        format!("{new_stem}.{extension}")
    };
    path.parent().unwrap_or(Path::new("")).join(file)
}

/// Monta o plano de renomeação da coleção inteira (o "propagar padrão").
/// A numeração do episódio vem do arquivo quando existe; senão, da ordem
/// natural dentro da temporada.
async fn build_plan(pool: &PgPool, collection_id: i32, pattern: &str) -> Result<Vec<RenamePlanItem>, String> {
    let (title, _) = repo::fetch_collection_title_type(pool, collection_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Coleção {collection_id} não encontrada"))?;
    let series = sanitize(&title);

    let seasons = repo::fetch_seasons(pool, collection_id).await.map_err(|e| e.to_string())?;
    let mut videos = repo::fetch_collection_videos(pool, collection_id)
        .await
        .map_err(|e| e.to_string())?;
    videos.sort_by(|a, b| natural_sort::compare(&a.file_path, &b.file_path));

    let mut plan = Vec::new();
    let mut sequence_by_season: std::collections::HashMap<Option<i32>, i32> = std::collections::HashMap::new();

    for video in &videos {
        let counter = sequence_by_season.entry(video.season_id).or_insert(0);
        *counter += 1;
        let episode = video.episode_number.unwrap_or(*counter);
        let season_number = video
            .season_id
            .and_then(|id| seasons.iter().find(|s| s.id == id))
            .and_then(|s| s.season_number);

        let new_stem = sanitize(&render_pattern(
            pattern,
            &series,
            season_number,
            episode,
            &file_stem(&video.file_path),
            &episode_name_for(video),
        ));
        if new_stem.is_empty() {
            continue;
        }

        let destination = target_path(&video.file_path, &new_stem);
        let current_name = file_name(&video.file_path);
        let new_name = destination
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let unchanged = new_name == current_name;
        let conflict = !unchanged && destination.exists();

        plan.push(RenamePlanItem { video_id: video.id, current_name, new_name, conflict, unchanged });
    }
    Ok(plan)
}

/// Pré-visualização: mostra de → para sem tocar em nada no disco.
#[tauri::command]
pub async fn preview_rename(
    collection_id: i32,
    pattern: String,
    pool: State<'_, PgPool>,
) -> Result<Vec<RenamePlanItem>, String> {
    build_plan(&pool, collection_id, &pattern).await
}

/// Aplica a renomeação. O plano é recalculado aqui (não confia no cliente) e
/// itens em conflito ou sem mudança são pulados.
#[tauri::command]
pub async fn apply_rename(
    collection_id: i32,
    pattern: String,
    pool: State<'_, PgPool>,
) -> Result<RenameOutcome, String> {
    let plan = build_plan(&pool, collection_id, &pattern).await?;
    let mut outcome = RenameOutcome { renamed: 0, skipped: 0, errors: Vec::new() };

    for item in plan {
        if item.unchanged || item.conflict {
            outcome.skipped += 1;
            continue;
        }
        let Some(video) = repo::fetch_video(&pool, item.video_id).await.map_err(|e| e.to_string())? else {
            outcome.skipped += 1;
            continue;
        };
        let destination = target_path(&video.file_path, &file_stem(&item.new_name));
        if destination.exists() {
            outcome.skipped += 1;
            continue;
        }
        match std::fs::rename(&video.file_path, &destination) {
            Ok(()) => {
                let new_path = destination.to_string_lossy().to_string();
                let display = file_stem(&new_path);
                repo::update_video_file(&pool, item.video_id, &new_path, &display)
                    .await
                    .map_err(|e| e.to_string())?;
                outcome.renamed += 1;
            }
            Err(e) => outcome.errors.push(format!("{}: {e}", item.current_name)),
        }
    }
    Ok(outcome)
}

// ---------- Nomes dos episódios ----------

/// Nome de episódio proposto pelo parser (nada é gravado até confirmar).
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeNamePlanItem {
    pub video_id: i32,
    pub current_name: String,
    pub new_name: String,
    /// Nome já definido pelo usuário — o parser não mexe.
    pub locked: bool,
    pub unchanged: bool,
}

async fn build_name_plan(pool: &PgPool, collection_id: i32) -> Result<Vec<EpisodeNamePlanItem>, String> {
    let mut videos = repo::fetch_collection_videos(pool, collection_id)
        .await
        .map_err(|e| e.to_string())?;
    videos.sort_by(|a, b| natural_sort::compare(&a.file_path, &b.file_path));

    Ok(videos
        .into_iter()
        .filter_map(|video| {
            let parsed = release_name::parse_episode_title(&file_stem(&video.file_path))?;
            let unchanged = parsed == video.display_name;
            Some(EpisodeNamePlanItem {
                video_id: video.id,
                current_name: video.display_name,
                new_name: parsed,
                locked: video.display_name_locked,
                unchanged,
            })
        })
        .collect())
}

/// Pré-visualiza os nomes que o parser consegue separar do nome do arquivo.
#[tauri::command]
pub async fn preview_episode_names(
    collection_id: i32,
    pool: State<'_, PgPool>,
) -> Result<Vec<EpisodeNamePlanItem>, String> {
    build_name_plan(&pool, collection_id).await
}

/// Grava os nomes extraídos (pula os que o usuário já definiu à mão).
#[tauri::command]
pub async fn apply_episode_names(
    collection_id: i32,
    pool: State<'_, PgPool>,
) -> Result<usize, String> {
    let plan = build_name_plan(&pool, collection_id).await?;
    let mut applied = 0;
    for item in plan {
        if item.locked || item.unchanged {
            continue;
        }
        repo::set_video_display_name(&pool, item.video_id, &item.new_name)
            .await
            .map_err(|e| e.to_string())?;
        applied += 1;
    }
    Ok(applied)
}

/// Edição manual do nome de um episódio (as exceções que o parser não pega).
#[tauri::command]
pub async fn set_video_display_name(
    video_id: i32,
    display_name: String,
    pool: State<'_, PgPool>,
) -> Result<(), String> {
    let name = display_name.trim().to_string();
    if name.is_empty() {
        return Err("Informe um nome para o episódio".to_string());
    }
    repo::set_video_display_name(&pool, video_id, &name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_collection_title(
    collection_id: i32,
    title: String,
    pool: State<'_, PgPool>,
) -> Result<(), String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("Informe um nome para a série".to_string());
    }
    repo::set_collection_title(&pool, collection_id, &title)
        .await
        .map_err(|e| e.to_string())
}

/// Catalogação manual: corrige o que o scan classificou errado.
#[tauri::command]
pub async fn set_collection_type(
    collection_id: i32,
    collection_type: String,
    pool: State<'_, PgPool>,
) -> Result<(), String> {
    let parsed = CollectionType::from_db_str(&collection_type)?;
    repo::set_collection_type(&pool, collection_id, parsed.as_db_str())
        .await
        .map_err(|e| e.to_string())
}

/// Agrupa: a coleção vira uma temporada dentro de outra (ex.: "T2" solta que
/// deveria ser a 2ª temporada de uma série já cadastrada).
#[tauri::command]
pub async fn merge_collection_into(
    source_id: i32,
    target_id: i32,
    pool: State<'_, PgPool>,
) -> Result<(), String> {
    if source_id == target_id {
        return Err("Escolha uma coleção diferente".to_string());
    }
    let (title, _) = repo::fetch_collection_title_type(&pool, source_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Coleção de origem não encontrada".to_string())?;
    let folder = repo::fetch_collection_folder(&pool, source_id)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| format!("merged-{source_id}"));

    repo::move_collection_content(&pool, source_id, target_id, &title, &folder)
        .await
        .map_err(|e| e.to_string())
}

/// Captura um frame do primeiro episódio e grava como capa da coleção.
#[tauri::command]
pub async fn generate_collection_poster(
    collection_id: i32,
    seconds: f64,
    app: AppHandle,
) -> Result<String, String> {
    let pool = app.state::<PgPool>();
    let mut videos = repo::fetch_collection_videos(&pool, collection_id)
        .await
        .map_err(|e| e.to_string())?;
    videos.sort_by(|a, b| natural_sort::compare(&a.file_path, &b.file_path));
    let first = videos.first().ok_or_else(|| "Coleção sem vídeos".to_string())?;
    let source = first.file_path.clone();
    let capture_seconds = first
        .duration_seconds
        .map(|duration| seconds.min((duration - 1.0).max(0.0)))
        .unwrap_or(seconds);

    let posters_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(POSTERS_SUBDIR);
    std::fs::create_dir_all(&posters_dir).map_err(|e| e.to_string())?;
    let poster_path = posters_dir.join(format!("{collection_id}.png"));

    let preview = app.state::<Arc<PreviewHandle>>().inner().clone();
    let png = tauri::async_runtime::spawn_blocking(move || preview.grab_poster(&source, capture_seconds))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Não foi possível capturar o frame nesse instante".to_string())?;

    std::fs::write(&poster_path, &png).map_err(|e| e.to_string())?;
    repo::set_collection_poster(&pool, collection_id, &poster_path.to_string_lossy())
        .await
        .map_err(|e| e.to_string())?;
    Ok(to_data_uri(&png))
}

// ---------- Miniaturas dos episódios ----------

fn thumb_path(app: &AppHandle, video_id: i32) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(THUMBS_SUBDIR);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{video_id}.png")))
}

/// Miniatura já gravada, se existir (a UI carrega só o que está em cache).
#[tauri::command]
pub async fn get_episode_thumbnail(video_id: i32, app: AppHandle) -> Result<Option<String>, String> {
    let path = thumb_path(&app, video_id)?;
    Ok(std::fs::read(path).ok().map(|png| to_data_uri(&png)))
}

/// Gera as miniaturas que faltam na coleção. Manual e em lote: decodificar um
/// frame por episódio é caro para rodar sozinho ao abrir a tela.
#[tauri::command]
pub async fn set_collection_thumbnails_visible(
    collection_id: i32,
    visible: bool,
    pool: State<'_, PgPool>,
) -> Result<(), String> {
    repo::set_collection_thumbnail_visibility(&pool, collection_id, visible)
        .await
        .map_err(|e| e.to_string())
}

async fn generate_thumbnail_for_video(video_id: i32, force: bool, app: &AppHandle) -> Result<bool, String> {
    let pool = app.state::<PgPool>();
    let video = repo::fetch_video(&pool, video_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Vídeo {video_id} não encontrado"))?;
    let destino = thumb_path(app, video.id)?;
    if destino.exists() && !force {
        return Ok(false);
    }
    if !Path::new(&video.file_path).is_file() {
        return Ok(false);
    }
    let seconds = video
        .duration_seconds
        .map(|d| d * THUMB_POSITION_RATIO)
        .unwrap_or(THUMB_FALLBACK_SECONDS);
    let preview = app.state::<Arc<PreviewHandle>>().inner().clone();
    let source = video.file_path.clone();
    let png = tauri::async_runtime::spawn_blocking(move || {
        preview.grab_still(&source, seconds, THUMB_WIDTH, THUMB_HEIGHT)
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some(png) = png {
        std::fs::write(&destino, png).map_err(|e| e.to_string())?;
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
pub async fn generate_episode_thumbnail(video_id: i32, force: bool, app: AppHandle) -> Result<bool, String> {
    generate_thumbnail_for_video(video_id, force, &app).await
}

#[tauri::command]
pub async fn generate_episode_thumbnails(
    collection_id: i32,
    force: bool,
    app: AppHandle,
    job: State<'_, ThumbnailJob>,
) -> Result<usize, String> {
    job.reset();
    let pool = app.state::<PgPool>();
    let mut videos = repo::fetch_collection_videos(&pool, collection_id)
        .await
        .map_err(|e| e.to_string())?;
    videos.sort_by(|a, b| natural_sort::compare(&a.file_path, &b.file_path));

    let preview = app.state::<Arc<PreviewHandle>>().inner().clone();
    let mut geradas = 0;
    let total = videos.len();
    let mut processadas = 0;
    let mut falhas = 0;
    let _ = app.emit(EVENT_THUMBNAIL_PROGRESS, serde_json::json!({
        "collectionId": collection_id,
        "processed": 0,
        "total": total,
        "generated": 0,
        "failed": 0,
        "current": "Iniciando...",
    }));

    for video in videos {
        if job.is_cancelled() {
            let _ = app.emit(EVENT_THUMBNAIL_PROGRESS, serde_json::json!({
                "collectionId": collection_id,
                "processed": processadas,
                "total": total,
                "generated": geradas,
                "failed": falhas,
                "current": "Interrompido",
                "cancelled": true,
            }));
            break;
        }
        let destino = thumb_path(&app, video.id)?;
        if destino.exists() && !force || !Path::new(&video.file_path).is_file() {
            processadas += 1;
            let _ = app.emit(EVENT_THUMBNAIL_PROGRESS, serde_json::json!({
                "collectionId": collection_id,
                "processed": processadas,
                "total": total,
                "generated": geradas,
                "failed": falhas,
                "current": video.display_name,
                "cancelled": false,
            }));
            continue;
        }
        let seconds = video
            .duration_seconds
            .map(|d| d * THUMB_POSITION_RATIO)
            .unwrap_or(THUMB_FALLBACK_SECONDS);
        let (preview, source) = (preview.clone(), video.file_path.clone());
        let png = tauri::async_runtime::spawn_blocking(move || {
            preview.grab_still(&source, seconds, THUMB_WIDTH, THUMB_HEIGHT)
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Some(png) = png {
            std::fs::write(&destino, &png).map_err(|e| e.to_string())?;
            geradas += 1;
        } else {
            falhas += 1;
        }
        processadas += 1;
        let _ = app.emit(EVENT_THUMBNAIL_PROGRESS, serde_json::json!({
            "collectionId": collection_id,
            "processed": processadas,
            "total": total,
            "generated": geradas,
            "failed": falhas,
            "current": video.display_name,
            "cancelled": false,
        }));
    }
    Ok(geradas)
}

#[tauri::command]
pub async fn cancel_episode_thumbnails(job: State<'_, ThumbnailJob>) -> Result<(), String> {
    job.request_cancel();
    Ok(())
}

/// Capa gravada, como data URI (evita configurar o protocolo de assets).
#[tauri::command]
pub async fn get_collection_poster(
    collection_id: i32,
    pool: State<'_, PgPool>,
) -> Result<Option<String>, String> {
    let Some(path) = repo::fetch_collection_poster(&pool, collection_id)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    Ok(std::fs::read(path).ok().map(|png| to_data_uri(&png)))
}
