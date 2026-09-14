//! Todo o SQL da aplicação vive aqui (spec §4.1 — nada de query fora de db/).

use sqlx::PgPool;

use crate::constants::{CONTINUE_WATCHING_LIMIT, WATCHED_THRESHOLD_RATIO};
use crate::db::models::{
    CollectionCard, ContinueWatchingItem, IntroMarkerDto, LibraryRoot, SeasonRow, UserSettings,
    VideoRow, VideoWithProgress,
};
use crate::scan::walker::{ScannedCollection, ScannedVideo};

type DbResult<T> = Result<T, sqlx::Error>;

// ---------- Scanner (upserts incrementais por path) ----------

pub async fn upsert_root(pool: &PgPool, path: &str) -> DbResult<i32> {
    let (id,): (i32,) = sqlx::query_as(
        "INSERT INTO library_roots (path) VALUES ($1)
         ON CONFLICT (path) DO UPDATE SET path = EXCLUDED.path
         RETURNING id",
    )
    .bind(path)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn upsert_collection(pool: &PgPool, root_id: i32, col: &ScannedCollection) -> DbResult<i32> {
    let type_str = if col.is_course { "course" } else if col.is_series { "series" } else { "movie" };
    let (id,): (i32,) = sqlx::query_as(
        "INSERT INTO collections (root_id, title, folder_path, type) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (folder_path) DO UPDATE
                     SET title = CASE WHEN collections.title_locked THEN collections.title ELSE EXCLUDED.title END,
                             type = CASE WHEN collections.type = 'course' THEN collections.type ELSE EXCLUDED.type END
         RETURNING id",
    )
    .bind(root_id)
    .bind(&col.title)
    .bind(&col.folder_path)
    .bind(type_str)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn upsert_season(
    pool: &PgPool,
    collection_id: i32,
    title: &str,
    folder_path: &str,
    season_number: Option<i32>,
) -> DbResult<i32> {
    let (id,): (i32,) = sqlx::query_as(
        "INSERT INTO seasons (collection_id, title, folder_path, season_number) VALUES ($1, $2, $3, $4)
         ON CONFLICT (folder_path) DO UPDATE
           SET title = EXCLUDED.title, season_number = EXCLUDED.season_number
         RETURNING id",
    )
    .bind(collection_id)
    .bind(title)
    .bind(folder_path)
    .bind(season_number)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn upsert_video(
    pool: &PgPool,
    collection_id: i32,
    season_id: Option<i32>,
    video: &ScannedVideo,
) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO videos (collection_id, season_id, file_path, display_name, episode_number)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (file_path) DO UPDATE
           SET collection_id = EXCLUDED.collection_id,
               season_id = EXCLUDED.season_id,
               display_name = CASE WHEN videos.display_name_locked
                                   THEN videos.display_name ELSE EXCLUDED.display_name END,
               episode_number = EXCLUDED.episode_number",
    )
    .bind(collection_id)
    .bind(season_id)
    .bind(&video.file_path)
    .bind(&video.display_name)
    .bind(video.episode_number)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove vídeos cujo arquivo sumiu do disco + temporadas/coleções vazias desta raiz.
pub async fn prune_missing(pool: &PgPool, root_id: i32) -> DbResult<usize> {
    let rows: Vec<(i32, String)> = sqlx::query_as(
        "SELECT v.id, v.file_path FROM videos v
         JOIN collections c ON c.id = v.collection_id
         WHERE c.root_id = $1",
    )
    .bind(root_id)
    .fetch_all(pool)
    .await?;

    let missing: Vec<i32> = rows
        .into_iter()
        .filter(|(_, path)| !std::path::Path::new(path).is_file())
        .map(|(id, _)| id)
        .collect();

    if !missing.is_empty() {
        sqlx::query("DELETE FROM videos WHERE id = ANY($1)")
            .bind(&missing)
            .execute(pool)
            .await?;
    }
    sqlx::query(
        "DELETE FROM seasons s WHERE NOT EXISTS (SELECT 1 FROM videos v WHERE v.season_id = s.id)",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "DELETE FROM collections c
         WHERE c.root_id = $1
           AND NOT EXISTS (SELECT 1 FROM videos v WHERE v.collection_id = c.id)",
    )
    .bind(root_id)
    .execute(pool)
    .await?;
    Ok(missing.len())
}

// ---------- Remoção ----------

/// Remove o registro do vídeo (o progresso vai junto por CASCADE).
pub async fn delete_video(pool: &PgPool, video_id: i32) -> DbResult<()> {
    sqlx::query("DELETE FROM videos WHERE id = $1")
        .bind(video_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Remove a coleção inteira (temporadas, vídeos, progresso e marcadores por CASCADE).
pub async fn delete_collection(pool: &PgPool, collection_id: i32) -> DbResult<()> {
    sqlx::query("DELETE FROM collections WHERE id = $1")
        .bind(collection_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn fetch_collection_folder(pool: &PgPool, collection_id: i32) -> DbResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT folder_path FROM collections WHERE id = $1")
        .bind(collection_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(path,)| path))
}

// ---------- Biblioteca ----------

pub async fn fetch_library_roots(pool: &PgPool) -> DbResult<Vec<LibraryRoot>> {
    sqlx::query_as("SELECT id, path, created_at FROM library_roots ORDER BY id")
        .fetch_all(pool)
        .await
}

pub async fn fetch_collections(pool: &PgPool) -> DbResult<Vec<CollectionCard>> {
    sqlx::query_as(
        "SELECT c.id, c.title, c.type AS collection_type,
                (c.poster_path IS NOT NULL) AS has_poster,
                COUNT(v.id) AS total_videos,
                COUNT(*) FILTER (WHERE p.watched) AS watched_videos,
                COALESCE(BOOL_OR(NOT COALESCE(p.watched, false) AND COALESCE(p.position_seconds, 0) > 0), false) AS has_in_progress
         FROM collections c
         LEFT JOIN videos v ON v.collection_id = c.id
         LEFT JOIN playback_progress p ON p.video_id = v.id
         GROUP BY c.id",
    )
    .fetch_all(pool)
    .await
}

/// Título manual (série/anime cujo nome não vem da pasta): trava contra re-scan.
pub async fn set_collection_title(pool: &PgPool, collection_id: i32, title: &str) -> DbResult<()> {
    sqlx::query("UPDATE collections SET title = $2, title_locked = TRUE WHERE id = $1")
        .bind(collection_id)
        .bind(title)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_collection_type(pool: &PgPool, collection_id: i32, collection_type: &str) -> DbResult<()> {
    sqlx::query("UPDATE collections SET type = $2 WHERE id = $1")
        .bind(collection_id)
        .bind(collection_type)
        .execute(pool)
        .await?;
    Ok(())
}

/// Move as temporadas e os vídeos de uma coleção para outra (agrupar manual).
/// Vídeos soltos da origem viram uma temporada com o título dela.
pub async fn move_collection_content(
    pool: &PgPool,
    source_id: i32,
    target_id: i32,
    season_title: &str,
    source_folder: &str,
) -> DbResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query("UPDATE seasons SET collection_id = $2 WHERE collection_id = $1")
        .bind(source_id)
        .bind(target_id)
        .execute(&mut *tx)
        .await?;

    let loose: Option<(i64,)> =
        sqlx::query_as("SELECT COUNT(*) FROM videos WHERE collection_id = $1 AND season_id IS NULL")
            .bind(source_id)
            .fetch_optional(&mut *tx)
            .await?;
    if loose.map(|(n,)| n > 0).unwrap_or(false) {
        let (season_id,): (i32,) = sqlx::query_as(
            "INSERT INTO seasons (collection_id, title, folder_path, season_number)
             VALUES ($1, $2, $3, NULL)
             ON CONFLICT (folder_path) DO UPDATE SET collection_id = EXCLUDED.collection_id
             RETURNING id",
        )
        .bind(target_id)
        .bind(season_title)
        .bind(source_folder)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("UPDATE videos SET season_id = $2 WHERE collection_id = $1 AND season_id IS NULL")
            .bind(source_id)
            .bind(season_id)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("UPDATE videos SET collection_id = $2 WHERE collection_id = $1")
        .bind(source_id)
        .bind(target_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM collections WHERE id = $1")
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await
}

pub async fn set_collection_poster(pool: &PgPool, collection_id: i32, poster_path: &str) -> DbResult<()> {
    sqlx::query("UPDATE collections SET poster_path = $2 WHERE id = $1")
        .bind(collection_id)
        .bind(poster_path)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn fetch_collection_poster(pool: &PgPool, collection_id: i32) -> DbResult<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT poster_path FROM collections WHERE id = $1")
            .bind(collection_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(path,)| path))
}

/// Após renomear o arquivo no disco, o registro passa a apontar para o novo
/// caminho. O nome de exibição só acompanha se não tiver sido definido pelo usuário.
pub async fn update_video_file(
    pool: &PgPool,
    video_id: i32,
    file_path: &str,
    display_name: &str,
) -> DbResult<()> {
    sqlx::query(
        "UPDATE videos
            SET file_path = $2,
                display_name = CASE WHEN display_name_locked THEN display_name ELSE $3 END
          WHERE id = $1",
    )
    .bind(video_id)
    .bind(file_path)
    .bind(display_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Nome de exibição definido pelo usuário (extraído ou digitado): fica travado.
pub async fn set_video_display_name(pool: &PgPool, video_id: i32, display_name: &str) -> DbResult<()> {
    sqlx::query("UPDATE videos SET display_name = $2, display_name_locked = TRUE WHERE id = $1")
        .bind(video_id)
        .bind(display_name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn fetch_collection_title_type(pool: &PgPool, collection_id: i32) -> DbResult<Option<(String, String)>> {
    sqlx::query_as("SELECT title, type FROM collections WHERE id = $1")
        .bind(collection_id)
        .fetch_optional(pool)
        .await
}

pub async fn fetch_collection_thumbnail_visibility(pool: &PgPool, collection_id: i32) -> DbResult<Option<bool>> {
    let row: Option<(bool,)> = sqlx::query_as("SELECT show_thumbnails FROM collections WHERE id = $1")
        .bind(collection_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(show,)| show))
}

pub async fn set_collection_thumbnail_visibility(pool: &PgPool, collection_id: i32, show: bool) -> DbResult<()> {
    sqlx::query("UPDATE collections SET show_thumbnails = $2 WHERE id = $1")
        .bind(collection_id)
        .bind(show)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn fetch_seasons(pool: &PgPool, collection_id: i32) -> DbResult<Vec<SeasonRow>> {
    sqlx::query_as(
        "SELECT id, collection_id, title, folder_path, season_number
         FROM seasons WHERE collection_id = $1",
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await
}

pub async fn fetch_collection_videos(pool: &PgPool, collection_id: i32) -> DbResult<Vec<VideoWithProgress>> {
    sqlx::query_as(
        "SELECT v.id, v.collection_id, v.season_id, v.file_path, v.display_name,
                v.display_name_locked, v.episode_number, v.duration_seconds,
                COALESCE(p.position_seconds, 0) AS position_seconds,
                COALESCE(p.watched, false) AS watched
         FROM videos v
         LEFT JOIN playback_progress p ON p.video_id = v.id
         WHERE v.collection_id = $1",
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await
}

pub async fn fetch_video(pool: &PgPool, video_id: i32) -> DbResult<Option<VideoRow>> {
    sqlx::query_as(
        "SELECT id, collection_id, season_id, file_path, display_name, episode_number, duration_seconds
         FROM videos WHERE id = $1",
    )
    .bind(video_id)
    .fetch_optional(pool)
    .await
}

pub async fn fetch_all_video_ids_in_collection(pool: &PgPool, collection_id: i32) -> DbResult<Vec<i32>> {
    let rows: Vec<(i32,)> = sqlx::query_as("SELECT id FROM videos WHERE collection_id = $1")
        .bind(collection_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Um vídeo qualquer da biblioteca — usado pelo autoteste de áudio.
pub async fn fetch_any_video(pool: &PgPool) -> DbResult<Option<VideoRow>> {
    sqlx::query_as(
        "SELECT id, collection_id, season_id, file_path, display_name, episode_number, duration_seconds
         FROM videos ORDER BY id LIMIT 1",
    )
    .fetch_optional(pool)
    .await
}

pub async fn fetch_continue_watching(pool: &PgPool) -> DbResult<Vec<ContinueWatchingItem>> {
    sqlx::query_as(
        "SELECT v.id AS video_id, v.display_name, v.duration_seconds, v.collection_id,
                c.title AS collection_title, p.position_seconds
         FROM playback_progress p
         JOIN videos v ON v.id = p.video_id
         JOIN collections c ON c.id = v.collection_id
         WHERE p.watched = false AND p.position_seconds > 0
         ORDER BY p.last_played_at DESC NULLS LAST
         LIMIT $1",
    )
    .bind(CONTINUE_WATCHING_LIMIT)
    .fetch_all(pool)
    .await
}

// ---------- Progresso ----------

pub async fn fetch_resume_position(pool: &PgPool, video_id: i32) -> DbResult<f64> {
    let row: Option<(f64,)> =
        sqlx::query_as("SELECT position_seconds FROM playback_progress WHERE video_id = $1")
            .bind(video_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(p,)| p).unwrap_or(0.0))
}

/// UPSERT do progresso; aplica a regra de assistido (>= 90% da duração) e nunca "desassiste".
pub async fn save_progress(
    pool: &PgPool,
    video_id: i32,
    position_seconds: f64,
    duration_seconds: Option<f64>,
) -> DbResult<()> {
    let watched = duration_seconds
        .map(|d| d > 0.0 && position_seconds >= WATCHED_THRESHOLD_RATIO * d)
        .unwrap_or(false);
    sqlx::query(
        "INSERT INTO playback_progress (video_id, position_seconds, watched, last_played_at, updated_at)
         VALUES ($1, $2, $3, now(), now())
         ON CONFLICT (video_id) DO UPDATE
           SET position_seconds = EXCLUDED.position_seconds,
               watched = playback_progress.watched OR EXCLUDED.watched,
               last_played_at = now(),
               updated_at = now()",
    )
    .bind(video_id)
    .bind(position_seconds)
    .bind(watched)
    .execute(pool)
    .await?;
    Ok(())
}

/// Marca/desmarca assistido à mão. Ao marcar, a posição vai para o fim; ao
/// desmarcar, o progresso é zerado (serve como "rever do início").
pub async fn set_video_watched(pool: &PgPool, video_id: i32, watched: bool) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO playback_progress (video_id, position_seconds, watched, last_played_at, updated_at)
         VALUES ($1,
                 CASE WHEN $2 THEN COALESCE((SELECT duration_seconds FROM videos WHERE id = $1), 0) ELSE 0 END,
                 $2, now(), now())
         ON CONFLICT (video_id) DO UPDATE
           SET watched = EXCLUDED.watched,
               position_seconds = EXCLUDED.position_seconds,
               updated_at = now()",
    )
    .bind(video_id)
    .bind(watched)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_library_root(pool: &PgPool, root_id: i32) -> DbResult<()> {
    sqlx::query("DELETE FROM library_roots WHERE id = $1")
        .bind(root_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_video_duration(pool: &PgPool, video_id: i32, duration_seconds: f64) -> DbResult<()> {
    sqlx::query("UPDATE videos SET duration_seconds = $2 WHERE id = $1 AND duration_seconds IS NULL")
        .bind(video_id)
        .bind(duration_seconds)
        .execute(pool)
        .await?;
    Ok(())
}

// ---------- Intro markers ----------

/// Marker aplicável ao vídeo — prioridade: season_id > collection_id (spec §9).
pub async fn fetch_intro_marker_for_video(pool: &PgPool, video_id: i32) -> DbResult<Option<IntroMarkerDto>> {
    if let Some(marker) = fetch_video_intro_marker(pool, video_id).await? {
        return Ok(Some(marker));
    }
    sqlx::query_as(
        "SELECT im.start_seconds, im.end_seconds
         FROM intro_markers im
         JOIN videos v ON v.collection_id = im.collection_id
         WHERE v.id = $1 AND (im.season_id = v.season_id OR im.season_id IS NULL)
         ORDER BY im.season_id NULLS LAST
         LIMIT 1",
    )
    .bind(video_id)
    .fetch_optional(pool)
    .await
}

pub async fn fetch_video_intro_marker(pool: &PgPool, video_id: i32) -> DbResult<Option<IntroMarkerDto>> {
    sqlx::query_as(
        "SELECT start_seconds, end_seconds
         FROM video_intro_markers
         WHERE video_id = $1",
    )
    .bind(video_id)
    .fetch_optional(pool)
    .await
}

pub async fn upsert_video_intro_marker(
    pool: &PgPool,
    video_id: i32,
    start_seconds: f64,
    end_seconds: f64,
    confidence: f64,
) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO video_intro_markers (video_id, start_seconds, end_seconds, confidence)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (video_id) DO UPDATE
           SET start_seconds = EXCLUDED.start_seconds,
               end_seconds = EXCLUDED.end_seconds,
               confidence = EXCLUDED.confidence,
               source = 'detected',
               created_at = now()",
    )
    .bind(video_id)
    .bind(start_seconds)
    .bind(end_seconds)
    .bind(confidence)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn upsert_intro_marker(
    pool: &PgPool,
    collection_id: i32,
    season_id: Option<i32>,
    start_seconds: f64,
    end_seconds: f64,
) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO intro_markers (collection_id, season_id, start_seconds, end_seconds)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (collection_id, season_id) DO UPDATE
           SET start_seconds = EXCLUDED.start_seconds, end_seconds = EXCLUDED.end_seconds",
    )
    .bind(collection_id)
    .bind(season_id)
    .bind(start_seconds)
    .bind(end_seconds)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------- Settings ----------

pub async fn fetch_settings(pool: &PgPool) -> DbResult<UserSettings> {
    sqlx::query_as("SELECT id, auto_skip_intro FROM user_settings ORDER BY id LIMIT 1")
        .fetch_one(pool)
        .await
}

pub async fn update_auto_skip_intro(pool: &PgPool, enabled: bool) -> DbResult<()> {
    sqlx::query("UPDATE user_settings SET auto_skip_intro = $1")
        .bind(enabled)
        .execute(pool)
        .await?;
    Ok(())
}
