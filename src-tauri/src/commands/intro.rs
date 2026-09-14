use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::constants::{
    EVENT_INTRO_ANALYSIS_PROGRESS, INTRO_ANALYSIS_FRAME_HEIGHT, INTRO_ANALYSIS_FRAME_WIDTH,
    INTRO_ANALYSIS_HASH_DISTANCE, INTRO_ANALYSIS_MAX_EPISODES, INTRO_ANALYSIS_MAX_SECONDS,
    INTRO_ANALYSIS_MIN_INTERVAL_SECONDS, INTRO_ANALYSIS_MIN_MATCH_RATIO,
};
use crate::db::models::CollectionType;
use crate::db::repo;
use crate::scan::natural_sort;
use crate::vlc_engine::preview::PreviewHandle;

#[derive(Default)]
pub struct IntroAnalysisJob {
    cancel_requested: AtomicBool,
}

impl IntroAnalysisJob {
    fn reset(&self) {
        self.cancel_requested.store(false, Ordering::Release);
    }

    fn cancel(&self) {
        self.cancel_requested.store(true, Ordering::Release);
    }

    fn cancelled(&self) -> bool {
        self.cancel_requested.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntroSuggestion {
    pub video_id: i32,
    pub display_name: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub confidence: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntroAnalysisRequest {
    pub collection_id: i32,
    pub video_ids: Vec<i32>,
    pub max_seconds: f64,
    pub sample_interval_seconds: f64,
}

fn average_hash(png: &[u8]) -> Option<u64> {
    let image = image::load_from_memory(png)
        .ok()?
        .resize_exact(8, 8, FilterType::Triangle)
        .to_luma8();
    let average = image.pixels().map(|pixel| pixel[0] as u64).sum::<u64>() / 64;
    let mut hash = 0u64;
    for (index, pixel) in image.pixels().enumerate() {
        if pixel[0] as u64 >= average {
            hash |= 1 << index;
        }
    }
    Some(hash)
}

fn hamming_distance(left: u64, right: u64) -> u32 {
    (left ^ right).count_ones()
}

async fn capture_hash(preview: Arc<PreviewHandle>, path: String, seconds: f64) -> Option<u64> {
    tauri::async_runtime::spawn_blocking(move || {
        preview
            .grab_still(&path, seconds, INTRO_ANALYSIS_FRAME_WIDTH, INTRO_ANALYSIS_FRAME_HEIGHT)
            .and_then(|png| average_hash(&png))
    })
    .await
    .ok()
    .flatten()
}

async fn sample_video(
    preview: Arc<PreviewHandle>,
    path: String,
    max_seconds: f64,
    interval: f64,
) -> Vec<(f64, u64)> {
    let mut samples = Vec::new();
    let mut seconds = 0.0;
    while seconds <= max_seconds {
        if let Some(hash) = capture_hash(preview.clone(), path.clone(), seconds).await {
            samples.push((seconds, hash));
        }
        seconds += interval;
    }
    samples
}

fn nearest_time(hash: u64, samples: &[(f64, u64)]) -> Option<(f64, u32)> {
    samples
        .iter()
        .map(|(seconds, candidate)| (*seconds, hamming_distance(hash, *candidate)))
        .min_by_key(|(_, distance)| *distance)
}

#[tauri::command]
pub async fn analyze_intro(
    request: IntroAnalysisRequest,
    app: AppHandle,
    job: State<'_, IntroAnalysisJob>,
) -> Result<Vec<IntroSuggestion>, String> {
    job.reset();
    let max_seconds = request.max_seconds.clamp(INTRO_ANALYSIS_MIN_INTERVAL_SECONDS, INTRO_ANALYSIS_MAX_SECONDS);
    let interval = request
        .sample_interval_seconds
        .max(INTRO_ANALYSIS_MIN_INTERVAL_SECONDS)
        .min(max_seconds);
    let pool = app.state::<PgPool>();
    let (_, type_str) = repo::fetch_collection_title_type(&pool, request.collection_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Coleção não encontrada".to_string())?;
    if CollectionType::from_db_str(&type_str)? != CollectionType::Series {
        return Err("A análise de introdução está disponível somente para séries".to_string());
    }

    let mut videos = repo::fetch_collection_videos(&pool, request.collection_id)
        .await
        .map_err(|e| e.to_string())?;
    videos.retain(|video| request.video_ids.contains(&video.id));
    videos.sort_by(|left, right| natural_sort::compare(&left.file_path, &right.file_path));
    if videos.len() < 2 {
        return Err("Selecione pelo menos dois episódios para comparar".to_string());
    }
    if videos.len() > INTRO_ANALYSIS_MAX_EPISODES {
        return Err(format!("Selecione no máximo {INTRO_ANALYSIS_MAX_EPISODES} episódios por análise"));
    }

    let total = videos.len();
    let preview = app.state::<Arc<PreviewHandle>>().inner().clone();
    let mut sampled: HashMap<i32, Vec<(f64, u64)>> = HashMap::new();
    for (index, video) in videos.iter().enumerate() {
        if job.cancelled() {
            let _ = app.emit(EVENT_INTRO_ANALYSIS_PROGRESS, serde_json::json!({
                "collectionId": request.collection_id,
                "processed": index,
                "total": total,
                "current": "Interrompido",
                "videoId": null,
                "cancelled": true,
            }));
            return Ok(Vec::new());
        }
        let samples = sample_video(preview.clone(), video.file_path.clone(), max_seconds, interval).await;
        sampled.insert(video.id, samples);
        let _ = app.emit(EVENT_INTRO_ANALYSIS_PROGRESS, serde_json::json!({
            "collectionId": request.collection_id,
            "processed": index + 1,
            "total": total,
            "current": video.display_name,
            "videoId": video.id,
            "cancelled": false,
        }));
    }

    let reference = videos.first().ok_or_else(|| "Nenhum episódio selecionado".to_string())?;
    let reference_samples = sampled.get(&reference.id).cloned().unwrap_or_default();
    let required_matches = ((videos.len() as f64) * INTRO_ANALYSIS_MIN_MATCH_RATIO).ceil() as usize;
    let matching_reference_times: Vec<f64> = reference_samples
        .iter()
        .filter_map(|(seconds, hash)| {
            let matches = videos
                .iter()
                .filter_map(|video| sampled.get(&video.id).and_then(|samples| nearest_time(*hash, samples)))
                .filter(|(_, distance)| *distance <= INTRO_ANALYSIS_HASH_DISTANCE)
                .count();
            (matches >= required_matches).then_some(*seconds)
        })
        .collect();

    if matching_reference_times.len() < 2 {
        return Ok(Vec::new());
    }
    let start = *matching_reference_times.first().unwrap_or(&0.0);
    let end = matching_reference_times
        .last()
        .copied()
        .unwrap_or(start + interval)
        + interval;
    let mut suggestions = Vec::new();
    for video in videos {
        let samples = sampled.get(&video.id).cloned().unwrap_or_default();
        let matched_times: Vec<f64> = matching_reference_times
            .iter()
            .filter_map(|reference_time| {
                let reference_hash = reference_samples
                    .iter()
                    .find(|(seconds, _)| (*seconds - *reference_time).abs() < 0.001)
                    .map(|(_, hash)| *hash)?;
                let (seconds, distance) = nearest_time(reference_hash, &samples)?;
                (distance <= INTRO_ANALYSIS_HASH_DISTANCE).then_some(seconds)
            })
            .collect();
        let video_start = matched_times.iter().copied().reduce(f64::min).unwrap_or(start);
        let video_end = matched_times.iter().copied().reduce(f64::max).unwrap_or(end - interval) + interval;
        suggestions.push(IntroSuggestion {
            video_id: video.id,
            display_name: video.display_name,
            start_seconds: video_start,
            end_seconds: video_end,
            confidence: (matched_times.len() as f64 / matching_reference_times.len() as f64).min(1.0),
        });
    }
    Ok(suggestions)
}

#[tauri::command]
pub async fn cancel_intro_analysis(job: State<'_, IntroAnalysisJob>) -> Result<(), String> {
    job.cancel();
    Ok(())
}

#[tauri::command]
pub async fn accept_intro_suggestion(
    video_id: i32,
    start_seconds: f64,
    end_seconds: f64,
    confidence: f64,
    pool: State<'_, PgPool>,
) -> Result<(), String> {
    if !(start_seconds.is_finite() && end_seconds.is_finite() && end_seconds > start_seconds) {
        return Err("Intervalo de introdução inválido".to_string());
    }
    repo::upsert_video_intro_marker(&pool, video_id, start_seconds, end_seconds, confidence)
        .await
        .map_err(|e| e.to_string())
}
