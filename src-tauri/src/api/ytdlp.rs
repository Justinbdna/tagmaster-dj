//! Client fallback `yt-dlp` (YouTube & SoundCloud) — SPECIFICATIONS.md §4.2 & Issue #11.
//!
//! - Appel métadonnées uniquement (`--dump-json --no-playlist --skip-download --no-warnings`).
//! - Recherche ciblée sur YouTube (`ytsearch5:`) et SoundCloud (`scsearch3:`).
//! - Déclenchement automatique sur les versions protégées (`Live`, `Remix`, `Mashup`,
//!   `Sped Up`, `Chopped`, `Acoustic`, `Instrumental`) et lorsque Apple Music échoue.
//! - Anti-ban (pause aléatoire 1-2s entre processus), timeout strict (20s) et gestion
//!   sans crash si `yt-dlp` est absent du `PATH`.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rand::RngExt;
use serde::Deserialize;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::models::{MetadataSource, TrackCandidate};
use crate::tagger::rules::{
    RawMetadataInput, clean_track_label, detect_protected_keyword, normalize_apostrophes,
    normalize_for_match, normalize_track_metadata, split_artist_title,
};

pub const YTDLP_TIMEOUT_SECS: u64 = 20;
pub const RATE_LIMIT_MIN_MS: u64 = 1000;
pub const RATE_LIMIT_MAX_MS: u64 = 2000;
pub const MAX_YTDLP_CANDIDATES: usize = 5;

#[cfg(unix)]
const FALLBACK_YTDLP_PATHS: &[&str] = &[
    "/opt/homebrew/bin/yt-dlp",
    "/usr/local/bin/yt-dlp",
    "/usr/bin/yt-dlp",
];
#[cfg(not(unix))]
const FALLBACK_YTDLP_PATHS: &[&str] = &[];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YtDlpError {
    NotInstalled,
    Timeout(u64),
    ExecutionFailed(String),
}

impl fmt::Display for YtDlpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(
                f,
                "yt-dlp est introuvable dans le PATH (fallback YouTube / SoundCloud indisponible)."
            ),
            Self::Timeout(secs) => write!(
                f,
                "Timeout yt-dlp après {secs}s lors de la recherche YouTube / SoundCloud."
            ),
            Self::ExecutionFailed(msg) => write!(f, "Erreur yt-dlp : {msg}"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct YtDlpThumbnail {
    url: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    preference: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct YtDlpEntry {
    title: Option<String>,
    track: Option<String>,
    artist: Option<String>,
    creator: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    thumbnails: Option<Vec<YtDlpThumbnail>>,
    release_year: Option<u32>,
    upload_date: Option<String>,
    extractor_key: Option<String>,
    extractor: Option<String>,
    webpage_url: Option<String>,
}

fn rate_limiter() -> &'static Mutex<Option<Instant>> {
    static LIMITER: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    LIMITER.get_or_init(|| Mutex::new(None))
}

async fn enforce_rate_limit() {
    let mut last_request = rate_limiter().lock().await;
    if let Some(prev) = *last_request {
        let min_gap_ms = rand::rng().random_range(RATE_LIMIT_MIN_MS..=RATE_LIMIT_MAX_MS);
        let min_gap = Duration::from_millis(min_gap_ms);
        let elapsed = prev.elapsed();
        if elapsed < min_gap {
            tokio::time::sleep(min_gap - elapsed).await;
        }
    }
    *last_request = Some(Instant::now());
}

/// Résout le binaire `yt-dlp` depuis une variable `PATH` explicite et une liste de chemins fallback.
pub fn resolve_ytdlp_binary_from(
    path_var: Option<&OsStr>,
    fallback_paths: &[&str],
) -> Option<PathBuf> {
    if let Some(raw_path) = path_var {
        let exe_names: &[&str] = if cfg!(windows) {
            &["yt-dlp.exe", "yt-dlp"]
        } else {
            &["yt-dlp"]
        };

        for dir in std::env::split_paths(raw_path) {
            for name in exe_names {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    for &fallback in fallback_paths {
        let candidate = PathBuf::from(fallback);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

/// Localise `yt-dlp` dans le `PATH` système ou dans les emplacements Homebrew/macOS usuels.
pub fn resolve_ytdlp_binary() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH");
    resolve_ytdlp_binary_from(path_var.as_deref(), FALLBACK_YTDLP_PATHS)
}

fn clean_person_field(raw: &str) -> String {
    let normalized = normalize_apostrophes(raw)
        .replace('，', ", ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    normalized
        .strip_suffix(" - Topic")
        .or_else(|| normalized.strip_suffix(" - topic"))
        .unwrap_or(&normalized)
        .trim()
        .to_string()
}

fn detect_platform(entry: &YtDlpEntry, default_platform: &str) -> String {
    let haystack = [
        entry.extractor_key.as_deref(),
        entry.extractor.as_deref(),
        entry.webpage_url.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_ascii_lowercase();

    if haystack.contains("soundcloud") {
        "SoundCloud".to_string()
    } else if haystack.contains("youtube") || haystack.contains("youtu.be") {
        "YouTube".to_string()
    } else {
        default_platform.to_string()
    }
}

fn upgrade_soundcloud_artwork_url(url: &str) -> String {
    if !url.contains("sndcdn.com") || url.contains("-original.") {
        return url.to_string();
    }

    for suffix in ["-large.", "-t200x200.", "-t300x300."] {
        if url.contains(suffix) {
            return url.replace(suffix, "-t500x500.");
        }
    }

    url.to_string()
}

fn is_webp_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains(".webp") || lower.contains("/vi_webp/")
}

fn select_hd_thumbnail(entry: &YtDlpEntry) -> Option<String> {
    if let Some(thumbs) = &entry.thumbnails {
        let mut ranked: Vec<((u8, u64, i32), String)> = thumbs
            .iter()
            .filter_map(|t| {
                let url = t.url.as_deref()?.trim();
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return None;
                }
                let area = u64::from(t.width.unwrap_or(0)) * u64::from(t.height.unwrap_or(0));
                let non_webp_priority = if is_webp_url(url) { 0 } else { 1 };
                let pref = t.preference.unwrap_or(0);
                Some((
                    (non_webp_priority, area, pref),
                    upgrade_soundcloud_artwork_url(url),
                ))
            })
            .collect();

        ranked.sort_by(|a, b| b.0.cmp(&a.0));
        if let Some((_, best_url)) = ranked.into_iter().next() {
            return Some(best_url);
        }
    }

    entry
        .thumbnail
        .as_deref()
        .map(str::trim)
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .map(upgrade_soundcloud_artwork_url)
}

fn extract_ytdlp_year(entry: &YtDlpEntry) -> Option<u32> {
    if let Some(y) = entry.release_year
        && (1900..=2100).contains(&y)
    {
        return Some(y);
    }

    entry
        .upload_date
        .as_deref()
        .and_then(|d| d.get(0..4))
        .and_then(|y| y.parse::<u32>().ok())
        .filter(|y| (1900..=2100).contains(y))
}

fn extract_artist_and_title(entry: &YtDlpEntry) -> Option<(String, String)> {
    let fallback_artist = [
        entry.artist.as_deref(),
        entry.creator.as_deref(),
        entry.uploader.as_deref(),
        entry.channel.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(clean_person_field)
    .find(|s| !s.is_empty());

    if let Some(raw_track) = entry.track.as_deref() {
        let clean_track = clean_track_label(raw_track);
        if let Some((parsed_artist, parsed_title)) = split_artist_title(&clean_track) {
            return Some((clean_person_field(&parsed_artist), parsed_title));
        }
        if let Some(raw_artist) = fallback_artist.as_deref()
            && !clean_track.is_empty()
        {
            return Some((raw_artist.to_string(), clean_track));
        }
    }

    let raw_title = entry.title.as_deref()?.trim();
    if raw_title.is_empty() {
        return None;
    }

    let cleaned_title = clean_track_label(raw_title);
    if cleaned_title.is_empty() {
        return None;
    }

    if let Some((parsed_artist, parsed_title)) = split_artist_title(&cleaned_title) {
        return Some((clean_person_field(&parsed_artist), parsed_title));
    }

    Some((
        fallback_artist.unwrap_or_else(|| "Artiste inconnu".to_string()),
        cleaned_title,
    ))
}

fn score_ytdlp_candidate(query: &str, artist: &str, title: &str) -> f32 {
    let norm_query = normalize_for_match(query);
    let cand_full = format!("{artist} {title}");
    let norm_cand = normalize_for_match(&cand_full);

    if norm_query.is_empty() || norm_cand.is_empty() {
        return 0.45;
    }

    let query_tokens: Vec<&str> = norm_query.split_whitespace().collect();
    let cand_tokens: Vec<&str> = norm_cand.split_whitespace().collect();

    let matched = query_tokens
        .iter()
        .filter(|tok| cand_tokens.contains(tok))
        .count();

    let recall = matched as f32 / query_tokens.len().max(1) as f32;
    let dice = (2.0 * matched as f32) / (query_tokens.len() + cand_tokens.len()).max(1) as f32;
    let mut score = recall * 0.65 + dice * 0.25;

    if let Some(expected_kw) = detect_protected_keyword(query) {
        if detect_protected_keyword(title) == Some(expected_kw) {
            score += 0.14;
        } else {
            score *= 0.80;
        }
    }

    score.clamp(0.15, 0.97)
}

/// Parse la sortie NDJSON produite par `yt-dlp --dump-json` en candidats normalisés.
pub fn parse_ytdlp_ndjson_lines(
    stdout: &str,
    default_platform: &str,
    query: &str,
) -> Vec<TrackCandidate> {
    let mut candidates = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(entry) = serde_json::from_str::<YtDlpEntry>(trimmed) else {
            continue;
        };

        let Some((artist, title)) = extract_artist_and_title(&entry) else {
            continue;
        };

        let platform = detect_platform(&entry, default_platform);
        let thumbnail = select_hd_thumbnail(&entry);
        let year = extract_ytdlp_year(&entry);
        let duration_secs = entry
            .duration
            .filter(|d| d.is_finite() && *d > 0.0)
            .map(|d| d.round() as u64);
        let score = score_ytdlp_candidate(query, &artist, &title);

        let normalized = normalize_track_metadata(RawMetadataInput {
            title,
            artist,
            collection_artist: None,
            album: entry
                .album
                .as_deref()
                .map(clean_person_field)
                .filter(|s| !s.is_empty()),
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            year,
            genre: None,
            artwork_url: thumbnail,
            is_compilation: false,
            source: MetadataSource::YtDlp,
        });

        candidates.push(TrackCandidate {
            title: normalized.title,
            artist: normalized.artist,
            album_artist: normalized.album_artist,
            album: normalized.album,
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            year: normalized.year,
            genre: normalized.genre,
            artwork_url: normalized.artwork_url,
            is_compilation: normalized.is_compilation,
            storefront: platform.clone(),
            confidence_score: score,
            source: Some(MetadataSource::YtDlp),
            platform: Some(platform),
            duration_secs,
        });
    }

    candidates
}

async fn run_ytdlp_platform_search(
    binary: &PathBuf,
    search_expr: &str,
    default_platform: &str,
    raw_query: &str,
) -> Result<Vec<TrackCandidate>, YtDlpError> {
    enforce_rate_limit().await;

    let mut cmd = Command::new(binary);
    cmd.args([
        "--dump-json",
        "--no-playlist",
        "--skip-download",
        "--no-warnings",
        "--ignore-errors",
        "--",
        search_expr,
    ]);
    cmd.kill_on_drop(true);

    let output = tokio::time::timeout(Duration::from_secs(YTDLP_TIMEOUT_SECS), cmd.output())
        .await
        .map_err(|_| YtDlpError::Timeout(YTDLP_TIMEOUT_SECS))?
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                YtDlpError::NotInstalled
            } else {
                YtDlpError::ExecutionFailed(err.to_string())
            }
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = parse_ytdlp_ndjson_lines(&stdout, default_platform, raw_query);

    if parsed.is_empty() && !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            return Err(YtDlpError::ExecutionFailed(stderr));
        }
    }

    Ok(parsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YtDlpTarget {
    Both,
    Youtube,
    Soundcloud,
}

pub fn build_search_specs(
    target: YtDlpTarget,
    cleaned_query: &str,
) -> Vec<(String, &'static str)> {
    match target {
        YtDlpTarget::Both => vec![
            (format!("ytsearch5:{cleaned_query}"), "YouTube"),
            (format!("scsearch3:{cleaned_query}"), "SoundCloud"),
        ],
        YtDlpTarget::Youtube => vec![(format!("ytsearch5:{cleaned_query}"), "YouTube")],
        YtDlpTarget::Soundcloud => vec![(format!("scsearch5:{cleaned_query}"), "SoundCloud")],
    }
}

async fn search_candidates_with_binary_for_target(
    binary: Option<PathBuf>,
    raw_query: &str,
    target: YtDlpTarget,
) -> Result<Vec<TrackCandidate>, YtDlpError> {
    let cleaned_query = raw_query.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned_query.is_empty() {
        return Ok(Vec::new());
    }

    let Some(bin_path) = binary else {
        return Err(YtDlpError::NotInstalled);
    };

    let specs = build_search_specs(target, &cleaned_query);
    let mut combined: Vec<TrackCandidate> = Vec::new();
    let mut last_err: Option<YtDlpError> = None;

    for (expr, platform) in specs {
        let res = run_ytdlp_platform_search(&bin_path, &expr, platform, &cleaned_query).await;
        if matches!(res, Err(YtDlpError::NotInstalled)) {
            return Err(YtDlpError::NotInstalled);
        }
        match res {
            Ok(cands) => combined.extend(cands),
            Err(err) => last_err = Some(err),
        }
    }

    if combined.is_empty()
        && let Some(err) = last_err
    {
        return Err(err);
    }

    let mut deduped: HashMap<(String, String, String), TrackCandidate> = HashMap::new();
    for cand in combined {
        let key = (
            normalize_for_match(&cand.artist),
            normalize_for_match(&cand.title),
            cand.platform.clone().unwrap_or_default(),
        );
        match deduped.get_mut(&key) {
            Some(existing) if existing.confidence_score >= cand.confidence_score => {}
            Some(existing) => *existing = cand,
            None => {
                deduped.insert(key, cand);
            }
        }
    }

    let mut ranked: Vec<TrackCandidate> = deduped.into_values().collect();
    ranked.sort_by(|a, b| b.confidence_score.total_cmp(&a.confidence_score));
    ranked.truncate(MAX_YTDLP_CANDIDATES);
    Ok(ranked)
}

#[cfg(test)]
async fn search_candidates_with_binary(
    binary: Option<PathBuf>,
    raw_query: &str,
) -> Result<Vec<TrackCandidate>, YtDlpError> {
    search_candidates_with_binary_for_target(binary, raw_query, YtDlpTarget::Both).await
}

/// Recherche des métadonnées sur YouTube (`ytsearch5`) et SoundCloud (`scsearch3`) via `yt-dlp`.
pub async fn search_candidates(raw_query: &str) -> Result<Vec<TrackCandidate>, YtDlpError> {
    search_candidates_with_binary_for_target(resolve_ytdlp_binary(), raw_query, YtDlpTarget::Both)
        .await
}

/// Recherche uniquement sur YouTube (`ytsearch5`) via `yt-dlp`.
pub async fn search_youtube_candidates(raw_query: &str) -> Result<Vec<TrackCandidate>, YtDlpError> {
    search_candidates_with_binary_for_target(
        resolve_ytdlp_binary(),
        raw_query,
        YtDlpTarget::Youtube,
    )
    .await
}

/// Recherche uniquement sur SoundCloud (`scsearch5`) via `yt-dlp`.
pub async fn search_soundcloud_candidates(
    raw_query: &str,
) -> Result<Vec<TrackCandidate>, YtDlpError> {
    search_candidates_with_binary_for_target(
        resolve_ytdlp_binary(),
        raw_query,
        YtDlpTarget::Soundcloud,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_youtube_and_soundcloud_ndjson_with_hd_artwork_and_protected_keywords() {
        let yt_json = r#"{"title":"Daft Punk - Get Lucky (Remix) [Official Audio]","uploader":"Daft Punk - Topic","duration":633,"thumbnails":[{"url":"https://i.ytimg.com/vi_webp/abc/maxresdefault.webp","width":1280,"height":720},{"url":"https://i.ytimg.com/vi/abc/hqdefault.jpg","width":480,"height":360},{"url":"https://i.ytimg.com/vi/abc/maxresdefault.jpg","width":1280,"height":720}],"release_year":2013,"extractor_key":"Youtube","webpage_url":"https://www.youtube.com/watch?v=abc"}"#;
        let sc_json = r#"{"title":"Get Lucky (Daft Punk Remix)","artist":"Daft Punk， Pharrell Williams","track":"Get Lucky (Daft Punk Remix)","duration":30.0,"thumbnail":"https://i1.sndcdn.com/artworks-fffDPoUMxtc6-0-large.jpg","upload_date":"20160329","extractor_key":"Soundcloud","webpage_url":"https://soundcloud.com/daft/get-lucky-remix"}"#;

        let yt = parse_ytdlp_ndjson_lines(yt_json, "YouTube", "Daft Punk Get Lucky Remix");
        assert_eq!(yt.len(), 1);
        assert_eq!(yt[0].artist, "Daft Punk");
        assert_eq!(yt[0].title, "Get Lucky (Remix)");
        assert_eq!(yt[0].platform.as_deref(), Some("YouTube"));
        assert_eq!(yt[0].source, Some(MetadataSource::YtDlp));
        assert_eq!(yt[0].duration_secs, Some(633));
        assert_eq!(yt[0].year, Some(2013));
        assert_eq!(
            yt[0].artwork_url.as_deref(),
            Some("https://i.ytimg.com/vi/abc/maxresdefault.jpg")
        );

        let sc = parse_ytdlp_ndjson_lines(sc_json, "SoundCloud", "Daft Punk Get Lucky Remix");
        assert_eq!(sc.len(), 1);
        assert_eq!(sc[0].artist, "Daft Punk, Pharrell Williams");
        assert_eq!(sc[0].album_artist, "Daft Punk");
        assert_eq!(sc[0].title, "Get Lucky (Daft Punk Remix)");
        assert_eq!(sc[0].platform.as_deref(), Some("SoundCloud"));
        assert_eq!(sc[0].duration_secs, Some(30));
        assert_eq!(sc[0].year, Some(2016));
        assert_eq!(
            sc[0].artwork_url.as_deref(),
            Some("https://i1.sndcdn.com/artworks-fffDPoUMxtc6-0-t500x500.jpg")
        );

        let sc_reupload = r#"{"title":"SOPHIE - PONYBOY (SXTH LXX REMIX)","uploader":"Lucidd","track":"SOPHIE - PONYBOY (SXTH LXX REMIX)","duration":192.0,"extractor_key":"Soundcloud"}"#;
        let parsed_reupload =
            parse_ytdlp_ndjson_lines(sc_reupload, "SoundCloud", "SOPHIE PONYBOY REMIX");
        assert_eq!(parsed_reupload.len(), 1);
        assert_eq!(parsed_reupload[0].artist, "SOPHIE");
        assert_eq!(parsed_reupload[0].title, "PONYBOY (SXTH LXX REMIX)");
    }

    #[test]
    fn returns_not_installed_without_crashing_when_ytdlp_missing_from_path() {
        let missing = resolve_ytdlp_binary_from(
            Some(OsStr::new("/nonexistent_tagmaster_dir")),
            &["/nonexistent_tagmaster_dir/yt-dlp"],
        );
        assert!(missing.is_none());

        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let res = rt.block_on(search_candidates_with_binary(
            None,
            "Daft Punk Get Lucky Remix",
        ));
        assert_eq!(res, Err(YtDlpError::NotInstalled));
        assert!(res.unwrap_err().to_string().contains("PATH"));
    }
}
