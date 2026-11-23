//! Sécurité Windows : protection contre la limite de 256 caractères des chemins physiques
//! et nettoyage des caractères interdits dans les noms de fichiers (`< > : " / \ | ? *`).

#[allow(dead_code)]
pub const WINDOWS_MAX_PATH_SAFE: usize = 240;

#[allow(dead_code)]
pub fn sanitize_filename_segment(segment: &str) -> String {
    segment
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .to_string()
}
