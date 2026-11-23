//! Sanitizer de caractères spéciaux / emojis pour profils rétro iPod & PSP 3000 (SPECIFICATIONS.md §4.3).

#[allow(dead_code)]
pub fn sanitize_for_retro_device(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii() || c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .trim()
        .to_string()
}
