//! Module de nettoyage ("Table Rase") et d'écriture des tags audio (SPECIFICATIONS.md §4).
//!
//! Sprint 1 : Intégration de `lofty` pour la suppression totale des anciens tags
//! publicitaires et l'écriture propre des tags ID3 / MP4 / FLAC + pochettes.

pub mod rules;
pub mod sanitizer;
