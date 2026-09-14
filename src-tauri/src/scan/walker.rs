//! Scan recursivo do diretório raiz (spec §7). Só filesystem — nada de banco aqui.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::natural_sort;
use super::release_name;
use super::release_name::{episode_number, first_number, prettify};
use crate::constants::VIDEO_EXTENSIONS;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub collections: usize,
    pub seasons: usize,
    pub videos: usize,
    pub removed: usize,
}

#[derive(Debug)]
pub struct ScannedVideo {
    pub file_path: String,
    pub display_name: String,
    pub episode_number: Option<i32>,
}

#[derive(Debug)]
pub struct ScannedSeason {
    pub folder_path: String,
    pub title: String,
    pub season_number: Option<i32>,
    pub videos: Vec<ScannedVideo>,
}

#[derive(Debug)]
pub struct ScannedCollection {
    pub folder_path: String,
    pub title: String,
    pub is_series: bool,
    pub is_course: bool,
    pub seasons: Vec<ScannedSeason>,
    pub loose_videos: Vec<ScannedVideo>,
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}


fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Todos os vídeos sob `dir` (recursivo), em ordem natural de caminho.
fn collect_videos_recursive(dir: &Path, acc: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_videos_recursive(&path, acc)?;
        } else if is_video(&path) {
            acc.push(path);
        }
    }
    Ok(())
}

fn to_scanned_videos(mut paths: Vec<PathBuf>) -> Vec<ScannedVideo> {
    paths.sort_by(|a, b| natural_sort::compare(&a.to_string_lossy(), &b.to_string_lossy()));
    paths
        .into_iter()
        .map(|p| {
            let stem = file_stem(&p);
            ScannedVideo {
                file_path: p.to_string_lossy().to_string(),
                episode_number: episode_number(&stem),
                display_name: prettify(&stem),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(files: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("moovia_test_{}", uuid()));
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"").unwrap();
        }
        root
    }

    fn uuid() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    /// Raiz apontada para a pasta da série, com temporadas em T1/T2 e uma pasta
    /// de release aninhada dentro de cada uma.
    #[test]
    fn raiz_com_pastas_de_temporada_vira_uma_colecao_so() {
        let root = make(&[
            r"T1\O.Rastreador.2024.S01-SF\ep01.mkv",
            r"T1\O.Rastreador.2024.S01-SF\ep02.mkv",
            r"T2\O.Rastreador.2024.S02-SF\ep01.mkv",
        ]);
        let collections = walk_root(&root).unwrap();

        assert_eq!(collections.len(), 1, "deveria ser uma coleção só");
        let series = &collections[0];
        assert!(series.is_series);
        assert_eq!(series.seasons.len(), 2);
        assert_eq!(series.seasons[0].title, "T1");
        assert_eq!(series.seasons[0].season_number, Some(1));
        assert_eq!(series.seasons[0].videos.len(), 2);
        assert_eq!(series.seasons[1].title, "T2");
        fs::remove_dir_all(&root).ok();
    }

    /// Raiz apontada para a biblioteca: cada pasta continua sendo uma série.
    #[test]
    fn raiz_de_biblioteca_mantem_uma_colecao_por_pasta() {
        let root = make(&[
            r"Serie A\T1\ep01.mkv",
            r"Serie B\ep01.mkv",
        ]);
        let collections = walk_root(&root).unwrap();

        assert_eq!(collections.len(), 2);
        assert_eq!(collections[0].title, "Serie A");
        assert_eq!(collections[0].seasons.len(), 1);
        assert_eq!(collections[1].title, "Serie B");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn raiz_cursos_mantem_um_card_por_curso_com_subpastas_recursivas() {
        let root = std::env::temp_dir().join(format!("moovia_test_cursos_{}", uuid()));
        let files = [
            "Aprenda Guitarra do Zero em 45 Dias/bloco 1/dia 1/aula 01.mp4",
            "Aprenda Guitarra do Zero em 45 Dias/bloco 1/dia 2/aula 02.mp4",
            "Blues sem Fronteiras/modulo 1/dia 1/aula 01.mp4",
        ];
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"").unwrap();
        }

        let cursos = root.with_file_name("cursos");
        fs::rename(&root, &cursos).unwrap();
        let collections = walk_root(&cursos).unwrap();

        assert_eq!(collections.len(), 2);
        assert!(collections.iter().all(|collection| collection.is_course));
        assert_eq!(collections[0].seasons.len(), 1);
        assert_eq!(collections[0].seasons[0].videos.len(), 2);
        assert_eq!(collections[1].seasons[0].videos.len(), 1);
        fs::remove_dir_all(&cursos).ok();
    }
}

/// Monta uma temporada a partir da pasta (recursivo: pastas de release aninhadas
/// dentro da temporada entram na mesma).
fn to_scanned_season(path: &Path, videos: Vec<PathBuf>) -> ScannedSeason {
    let title = dir_name(path);
    ScannedSeason {
        folder_path: path.to_string_lossy().to_string(),
        season_number: release_name::season_number(&title),
        title,
        videos: to_scanned_videos(videos),
    }
}

fn root_title(root: &Path) -> String {
    let title = dir_name(root);
    if title.is_empty() {
        root.to_string_lossy().to_string()
    } else {
        title
    }
}

fn is_courses_context(root: &Path) -> bool {
    root.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("cursos")
    })
}

/// Percorre a raiz: pasta de 1º nível = collection; subpasta com vídeo = season (series);
/// vídeos soltos sem subpastas de vídeo = movie.
///
/// Dois casos em que a RAIZ é a própria série (usuário aponta a pasta da série,
/// não a da biblioteca):
/// - vídeos soltos direto na raiz;
/// - todas as pastas de 1º nível são temporadas ("T1", "T2", "Season 3") — aí
///   elas viram temporadas de uma coleção só, em vez de coleções separadas.
pub fn walk_root(root: &Path) -> io::Result<Vec<ScannedCollection>> {
    let mut collection_dirs: Vec<PathBuf> = Vec::new();
    let mut root_loose_files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            collection_dirs.push(path);
        } else if is_video(&path) {
            root_loose_files.push(path);
        }
    }
    collection_dirs.sort_by(|a, b| natural_sort::compare(&a.to_string_lossy(), &b.to_string_lossy()));

    let mut collections = Vec::new();

    let courses_context = is_courses_context(root);
    let root_is_courses_folder = root_title(root).eq_ignore_ascii_case("cursos");

    if courses_context && !root_is_courses_folder {
        let mut seasons = Vec::new();
        for dir in &collection_dirs {
            let mut videos = Vec::new();
            collect_videos_recursive(dir, &mut videos)?;
            if !videos.is_empty() {
                seasons.push(to_scanned_season(dir, videos));
            }
        }
        if !seasons.is_empty() || !root_loose_files.is_empty() {
            return Ok(vec![ScannedCollection {
                folder_path: root.to_string_lossy().to_string(),
                title: root_title(root),
                is_series: false,
                is_course: true,
                seasons,
                loose_videos: to_scanned_videos(root_loose_files),
            }]);
        }
    }

    let root_is_series = !collection_dirs.is_empty()
        && collection_dirs.iter().all(|dir| release_name::is_season_folder(&dir_name(dir)));
    if root_is_series {
        let mut seasons = Vec::new();
        for dir in &collection_dirs {
            let mut videos = Vec::new();
            collect_videos_recursive(dir, &mut videos)?;
            if !videos.is_empty() {
                seasons.push(to_scanned_season(dir, videos));
            }
        }
        if !seasons.is_empty() {
            collections.push(ScannedCollection {
                folder_path: root.to_string_lossy().to_string(),
                title: root_title(root),
                is_series: true,
                is_course: false,
                seasons,
                loose_videos: to_scanned_videos(root_loose_files),
            });
            return Ok(collections);
        }
    }

    if !root_loose_files.is_empty() {
        collections.push(ScannedCollection {
            folder_path: root.to_string_lossy().to_string(),
            title: root_title(root),
            is_series: root_loose_files.len() > 1,
            is_course: false,
            seasons: Vec::new(),
            loose_videos: to_scanned_videos(root_loose_files),
        });
    }
    let courses_root = courses_context && root_is_courses_folder;
    for dir in collection_dirs {
        let mut season_dirs = Vec::new();
        let mut loose_files = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                let mut vids = Vec::new();
                collect_videos_recursive(&path, &mut vids)?;
                if !vids.is_empty() {
                    season_dirs.push((path, vids));
                }
            } else if is_video(&path) {
                loose_files.push(path);
            }
        }

        if season_dirs.is_empty() && loose_files.is_empty() {
            continue; // pasta sem nenhum vídeo não vira collection
        }

        season_dirs.sort_by(|a, b| natural_sort::compare(&a.0.to_string_lossy(), &b.0.to_string_lossy()));
        let is_series = !season_dirs.is_empty();
        let seasons = season_dirs
            .into_iter()
            .map(|(path, vids)| to_scanned_season(&path, vids))
            .collect();

        collections.push(ScannedCollection {
            folder_path: dir.to_string_lossy().to_string(),
            title: dir_name(&dir),
            is_series,
            is_course: courses_root,
            seasons,
            loose_videos: to_scanned_videos(loose_files),
        });
    }
    Ok(collections)
}

