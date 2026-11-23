//! Cascade de sources de métadonnées (SPECIFICATIONS.md §4.2).
//!
//! Ordre de priorité :
//! 1. Apple Music (iTunes Search API, région forcée US/UK, rate-limiting 1-2s)
//! 2. YouTube & SoundCloud via `yt-dlp` (remixes, lives, mashups, versions alternatives)
//! 3. Fallback pochette complémentaire
//! 4. Genius API (paroles, optionnel via `.env`)
//! 5. Discogs API (V2 / rips CD physiques, optionnel via `.env`)

pub mod apple_music;
pub mod discogs;
pub mod genius;
pub mod yt_dlp;
