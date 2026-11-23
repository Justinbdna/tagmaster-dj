//! Règles métier strictes de tagging (SPECIFICATIONS.md §4.3) :
//! - Featurings normalisés dans le titre (`Titre (feat. A & B)`) et `Album Artist` verrouillé.
//! - Singles : `Album` = nom du single.
//! - Compilations / BO : `Album Artist` = `Various Artists`.
//! - Multi-disques : `Disc Number` obligatoire (`1/2`, `2/2`).
//! - Mots-clés intouchables : empêcher Apple Music d'écraser une version alternative.

#[allow(dead_code)]
pub const PROTECTED_KEYWORDS: &[&str] = &[
    "live",
    "acoustic",
    "sped up",
    "chopped",
    "instrumental",
    "remix",
    "mashup",
];

#[allow(dead_code)]
pub fn contains_protected_keyword(input: &str) -> bool {
    let lower = input.to_lowercase();
    PROTECTED_KEYWORDS.iter().any(|kw| lower.contains(kw))
}
