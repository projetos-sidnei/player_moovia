//! Structs espelhando tabelas + DTOs enviados ao frontend (spec §4.1).
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Valor da coluna `collections.type` ('series' | 'movie' | 'course').
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionType {
    Series,
    Movie,
    Course,
}

impl CollectionType {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            CollectionType::Series => "series",
            CollectionType::Movie => "movie",
            CollectionType::Course => "course",
        }
    }

    pub fn from_db_str(value: &str) -> Result<Self, String> {
        match value {
            "series" => Ok(CollectionType::Series),
            "movie" => Ok(CollectionType::Movie),
            "course" => Ok(CollectionType::Course),
            other => Err(format!("collections.type inválido: {other}")),
        }
    }
}

impl TryFrom<String> for CollectionType {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_db_str(&value)
    }
}

// ---------- Linhas de tabela ----------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRoot {
    pub id: i32,
    pub path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SeasonRow {
    pub id: i32,
    pub collection_id: i32,
    pub title: String,
    pub folder_path: String,
    pub season_number: Option<i32>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct VideoRow {
    pub id: i32,
    pub collection_id: i32,
    pub season_id: Option<i32>,
    pub file_path: String,
    pub display_name: String,
    pub episode_number: Option<i32>,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    pub id: i32,
    pub auto_skip_intro: bool,
}

// ---------- DTOs para o frontend ----------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCard {
    pub id: i32,
    pub title: String,
    #[sqlx(try_from = "String")]
    pub collection_type: CollectionType,
    pub has_poster: bool,
    pub total_videos: i64,
    pub watched_videos: i64,
    pub has_in_progress: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct VideoWithProgress {
    pub id: i32,
    pub collection_id: i32,
    pub season_id: Option<i32>,
    pub file_path: String,
    pub display_name: String,
    /// Nome definido pelo usuário (extraído ou digitado) — imune ao re-scan.
    pub display_name_locked: bool,
    pub episode_number: Option<i32>,
    pub duration_seconds: Option<f64>,
    pub position_seconds: f64,
    pub watched: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ContinueWatchingItem {
    pub video_id: i32,
    pub display_name: String,
    pub duration_seconds: Option<f64>,
    pub collection_id: i32,
    pub collection_title: String,
    pub position_seconds: f64,
}

#[derive(Debug, Clone, Copy, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct IntroMarkerDto {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

/// Detalhe completo de uma coleção (tela 3 do MVP).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDetail {
    pub id: i32,
    pub title: String,
    pub collection_type: CollectionType,
    pub show_thumbnails: bool,
    pub seasons: Vec<SeasonWithVideos>,
    pub loose_videos: Vec<VideoWithProgress>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonWithVideos {
    pub season: SeasonRow,
    pub videos: Vec<VideoWithProgress>,
}
