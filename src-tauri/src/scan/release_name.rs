//! Leitura de nomes de arquivo no padrão de release:
//! `Série . SxxEyy . Nome do episódio . tags técnicas . -Grupo`

use std::sync::LazyLock;

use regex::Regex;

/// "S01E02", "s01.e02", "S01 E02"… — o episódio é o grupo depois do E.
pub static SEASON_EPISODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)s\d{1,3}[.\-_ ]?e(\d{1,4})").unwrap());

/// "Ep 2", "E02", "Episode 3", "Episódio 3"…
pub static EPISODE_WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:ep?|episode|epis[oó]dio)\.? ?(\d{1,4})\b").unwrap());

/// Pasta que nomeia uma temporada: "T1", "S02", "Season 17", "Temporada 2".
static SEASON_FOLDER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(t|s|season|temporada|parte|part|p)[ ._-]*\d{1,3}$").unwrap());

/// Resolução (1080p, 2160p…) e ano solto — sinalizam o fim do nome do episódio.
static RESOLUTION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{3,4}p$").unwrap());
static YEAR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(19|20)\d{2}$").unwrap());

/// Tudo a partir daqui é metadado de release, não faz parte do nome.
const RELEASE_TOKENS: &[&str] = &[
    // fonte
    "web", "dl", "webrip", "webdl", "bluray", "bdrip", "brrip", "hdtv", "dvdrip", "remux", "hdrip",
    // distribuidora
    "dsnp", "amzn", "nf", "hmax", "atvp", "hulu", "max", "crunchyroll", "cr",
    // codec
    "x264", "x265", "h264", "h265", "hevc", "avc", "xvid", "av1", "264", "265", "10bit", "8bit",
    // áudio
    "aac", "ac3", "eac3", "ddp", "dd", "dts", "truehd", "atmos", "flac", "opus", "mp3",
    // idioma / variantes
    "dual", "dublado", "legendado", "leg", "multi", "subbed", "dubbed",
    // outros
    "proper", "repack", "extended", "uncut", "internal", "hdr", "hdr10", "sdr", "dv", "imax",
];

fn is_release_token(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    RELEASE_TOKENS.contains(&lower.as_str()) || RESOLUTION_RE.is_match(&lower) || YEAR_RE.is_match(&lower)
}

/// "S01", "T2", "Season 17", "Temporada 3" dentro do nome da pasta.
static SEASON_NUMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:s|t|season|temporada)[ ._-]*(\d{1,3})\b").unwrap());

/// Número da temporada a partir do nome da pasta. Usa o marcador de temporada
/// antes de recorrer ao primeiro número — senão "O.Rastreador.2024.S01" viraria
/// temporada 2024.
pub fn season_number(name: &str) -> Option<i32> {
    SEASON_NUMBER_RE
        .captures(name)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
        .or_else(|| first_number(name))
}

/// A pasta é o nome de uma temporada (e não de uma série)?
pub fn is_season_folder(name: &str) -> bool {
    SEASON_FOLDER_RE.is_match(name.trim())
}

/// Número do episódio: padrão SxxEyy > "Ep N" > primeiro número do nome.
pub fn episode_number(name: &str) -> Option<i32> {
    SEASON_EPISODE_RE
        .captures(name)
        .or_else(|| EPISODE_WORD_RE.captures(name))
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
        .or_else(|| first_number(name))
}

/// Primeiro grupo de dígitos do nome — usado para temporada e como último recurso.
pub fn first_number(name: &str) -> Option<i32> {
    let digits: String = name
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Nome legível: pontos/underscores de nomes de release viram espaços.
pub fn prettify(stem: &str) -> String {
    let replaced: String = stem
        .chars()
        .map(|c| if c == '.' || c == '_' { ' ' } else { c })
        .collect();
    replaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn title_case(words: &[&str]) -> String {
    words
        .iter()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Nome do episódio: o que está entre o marcador SxxEyy e a primeira tag técnica.
/// `BLEACH...S01E01.THE.BLOOD.WARFARE.1080p.DSNP.WEB-DL` → `The Blood Warfare`.
/// Devolve None quando o arquivo não tem nome de episódio (só numeração).
pub fn parse_episode_title(stem: &str) -> Option<String> {
    let marker = SEASON_EPISODE_RE
        .find(stem)
        .or_else(|| EPISODE_WORD_RE.find(stem))?;
    let after = &stem[marker.end()..];

    let mut words: Vec<&str> = Vec::new();
    for token in after.split(['.', '_', ' ', '-']).filter(|t| !t.is_empty()) {
        if is_release_token(token) {
            break;
        }
        words.push(token);
    }
    // Números soltos que sobraram de "5.1"/"2.0" não fazem parte do nome.
    while words.last().is_some_and(|w| w.chars().all(|c| c.is_ascii_digit())) {
        words.pop();
    }
    if words.is_empty() {
        return None;
    }
    Some(title_case(&words))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrai_nome_de_release_com_tags() {
        let stem = "BLEACH.Thousand-Year.Blood.War.S01E01.THE.BLOOD.WARFARE.1080p.DSNP.WEB-DL";
        assert_eq!(parse_episode_title(stem).as_deref(), Some("The Blood Warfare"));
        assert_eq!(episode_number(stem), Some(1));
    }

    #[test]
    fn sem_nome_de_episodio_devolve_none() {
        let stem = "O.Rastreador.S01E02.1080p.H.265.WEB-DL.DUAL.5.1.SF";
        assert_eq!(parse_episode_title(stem), None);
        assert_eq!(episode_number(stem), Some(2));
    }

    #[test]
    fn temporada_vem_do_marcador_e_nao_do_ano() {
        assert_eq!(season_number("O.Rastreador.2024.S01.WEB-DL.1080p-SF"), Some(1));
        assert_eq!(season_number("T2"), Some(2));
        assert_eq!(season_number("Season 17"), Some(17));
        assert_eq!(season_number("Temporada 3"), Some(3));
        assert_eq!(season_number("P-2"), Some(2));
    }

    #[test]
    fn ignora_grupo_e_canais_de_audio() {
        let stem = "Serie.S02E10.O.Retorno.Do.Rei.2160p.AMZN.WEB-DL.DDP5.1-Grupo";
        assert_eq!(parse_episode_title(stem).as_deref(), Some("O Retorno Do Rei"));
    }
}
