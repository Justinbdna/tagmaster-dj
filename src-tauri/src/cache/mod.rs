//! Persistance d'état locale `cache.json` (SPECIFICATIONS.md §6).
//!
//! Sauvegarde après chaque piste traitée avec succès afin de permettre
//! la reprise après coupure réseau ou fermeture de l'application (Sprint 2).

#[allow(dead_code)]
pub const CACHE_FILENAME: &str = "cache.json";
