//! Module d'export sécurisé non destructif (SPECIFICATIONS.md §5).
//!
//! Ne jamais écraser les fichiers originaux : tous les exports sont écrits dans
//! `Documents/Tagmaster_DJ_Export` en mode Plat ou Dynamique.

pub mod path_guard;

#[allow(dead_code)]
pub const DEFAULT_EXPORT_DIR_NAME: &str = "Tagmaster_DJ_Export";
