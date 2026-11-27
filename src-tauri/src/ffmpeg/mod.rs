//! Moteur FFmpeg : détection, transcodage MP3 CBR 320 kbps et redimensionnement
//! de pochettes en JPEG baseline (SPECIFICATIONS.md §4.4 & §5.3 · Issues #15 & #16).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::apple_music::DownloadedArtwork;
use crate::models::{ExportFormatMode, FfmpegStatus, HardwareProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfmpegError {
    NotInstalled,
    ExecutionFailed(String),
}

impl std::fmt::Display for FfmpegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfmpegError::NotInstalled => write!(
                f,
                "FFmpeg introuvable dans le PATH : installez ffmpeg (ex. brew install ffmpeg) pour le transcodage MP3 320 kbps et le redimensionnement des pochettes."
            ),
            FfmpegError::ExecutionFailed(msg) => write!(f, "Erreur FFmpeg : {msg}"),
        }
    }
}

/// Détecte le binaire `ffmpeg` dans le `PATH`, Homebrew (macOS) et Winget/Chocolatey/Scoop (Windows).
pub fn resolve_ffmpeg_binary() -> Option<PathBuf> {
    let mut candidates: Vec<String> = vec!["ffmpeg".to_string()];

    #[cfg(unix)]
    {
        candidates.extend([
            "/opt/homebrew/bin/ffmpeg".to_string(),
            "/usr/local/bin/ffmpeg".to_string(),
            "/usr/bin/ffmpeg".to_string(),
        ]);
    }

    #[cfg(windows)]
    {
        candidates.push("ffmpeg.exe".to_string());
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_link = PathBuf::from(&local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Links")
                .join("ffmpeg.exe");
            candidates.push(winget_link.to_string_lossy().into_owned());
        }
        if let Ok(program_data) = std::env::var("ProgramData") {
            let choco = PathBuf::from(&program_data)
                .join("chocolatey")
                .join("bin")
                .join("ffmpeg.exe");
            candidates.push(choco.to_string_lossy().into_owned());
        }
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let scoop = PathBuf::from(&user_profile)
                .join("scoop")
                .join("shims")
                .join("ffmpeg.exe");
            candidates.push(scoop.to_string_lossy().into_owned());
        }
        candidates.push(r"C:\ffmpeg\bin\ffmpeg.exe".to_string());
    }

    let refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
    resolve_ffmpeg_from_candidates(&refs)
}

fn resolve_ffmpeg_from_candidates(candidates: &[&str]) -> Option<PathBuf> {
    for candidate in candidates {
        let status = Command::new(candidate)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if let Ok(exit) = status
            && exit.success()
        {
            return Some(PathBuf::from(candidate));
        }
    }
    None
}

/// Retourne l'état de disponibilité de `ffmpeg` pour l'interface Paramètres.
pub fn check_ffmpeg_status() -> FfmpegStatus {
    let Some(bin) = resolve_ffmpeg_binary() else {
        return FfmpegStatus {
            available: false,
            version: None,
            binary_path: None,
        };
    };

    let version = Command::new(&bin)
        .arg("-version")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|stdout| stdout.lines().next().map(str::trim).map(str::to_string));

    FfmpegStatus {
        available: true,
        version,
        binary_path: Some(bin.to_string_lossy().into_owned()),
    }
}

/// Détermine si un fichier source doit être transcodé en MP3 320 kbps selon le profil et le mode audio.
pub fn should_transcode_to_mp3(
    profile: &HardwareProfile,
    format_mode: &ExportFormatMode,
    source_ext: &str,
) -> bool {
    let ext = source_ext.trim().trim_start_matches('.').to_ascii_lowercase();
    match profile {
        HardwareProfile::Psp => true,
        HardwareProfile::Ipod => {
            *format_mode == ExportFormatMode::ForceMp3 || !matches!(ext.as_str(), "mp3" | "m4a" | "aac")
        }
        HardwareProfile::Standard => *format_mode == ExportFormatMode::ForceMp3,
    }
}

/// Taille maximale (en pixels) de la pochette carrée pour un profil matériel.
pub fn max_artwork_size_for_profile(profile: &HardwareProfile) -> u32 {
    match profile {
        HardwareProfile::Standard => 1000,
        HardwareProfile::Ipod => 600,
        HardwareProfile::Psp => 400,
    }
}

/// Indique si le profil exige une pochette strictement en JPEG baseline (pas de progressif ni WebP/PNG).
pub fn requires_baseline_jpeg(profile: &HardwareProfile) -> bool {
    matches!(profile, HardwareProfile::Ipod | HardwareProfile::Psp)
}

/// Vérifie au niveau des marqueurs binaires JPEG qu'une image est un JPEG baseline (`SOF0` `0xFF 0xC0`)
/// et non un JPEG progressif (`SOF2` `0xFF 0xC2`).
pub fn is_baseline_jpeg(bytes: &[u8]) -> bool {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return false;
    }

    let mut i = 2;
    let mut found_sof0 = false;

    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        while i < bytes.len() && bytes[i] == 0xFF {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let marker = bytes[i];
        i += 1;

        // Marqueurs sans longueur
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue;
        }

        if marker == 0xC0 {
            found_sof0 = true;
        } else if marker == 0xC2 {
            // Progressive DCT JPEG interdit sur iPod / PSP
            return false;
        }

        if i + 1 >= bytes.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        if seg_len < 2 {
            break;
        }
        i = i.saturating_add(seg_len);
    }

    found_sof0
}

/// Transcode un fichier audio source vers un MP3 CBR 320 kbps (`libmp3lame`, `44.1 kHz`, stéréo).
pub fn transcode_to_mp3_320(source_path: &Path, dest_path: &Path) -> Result<(), FfmpegError> {
    let bin = resolve_ffmpeg_binary().ok_or(FfmpegError::NotInstalled)?;
    transcode_to_mp3_320_with_binary(&bin, source_path, dest_path)
}

pub fn transcode_to_mp3_320_with_binary(
    ffmpeg_bin: &Path,
    source_path: &Path,
    dest_path: &Path,
) -> Result<(), FfmpegError> {
    let output = Command::new(ffmpeg_bin)
        .args(["-y", "-v", "error", "-i"])
        .arg(source_path)
        .args([
            "-vn",
            "-map_metadata",
            "-1",
            "-c:a",
            "libmp3lame",
            "-b:a",
            "320k",
            "-ar",
            "44100",
            "-ac",
            "2",
        ])
        .arg(dest_path)
        .output()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                FfmpegError::NotInstalled
            } else {
                FfmpegError::ExecutionFailed(err.to_string())
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(FfmpegError::ExecutionFailed(if stderr.is_empty() {
            format!("code de sortie {:?}", output.status.code())
        } else {
            stderr
        }));
    }

    Ok(())
}

/// Redimensionne une pochette en carré (`<= max_size_px`) et l'encode en **JPEG baseline** (`SOF0`).
pub fn resize_artwork_to_baseline_jpeg(
    artwork: &DownloadedArtwork,
    max_size_px: u32,
) -> Result<DownloadedArtwork, FfmpegError> {
    let bin = resolve_ffmpeg_binary().ok_or(FfmpegError::NotInstalled)?;
    resize_artwork_to_baseline_jpeg_with_binary(&bin, artwork, max_size_px)
}

pub fn resize_artwork_to_baseline_jpeg_with_binary(
    ffmpeg_bin: &Path,
    artwork: &DownloadedArtwork,
    max_size_px: u32,
) -> Result<DownloadedArtwork, FfmpegError> {
    if artwork.bytes.is_empty() {
        return Err(FfmpegError::ExecutionFailed(
            "Image pochette vide.".to_string(),
        ));
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let in_ext = if artwork.mime_type.contains("png") {
        "png"
    } else if artwork.mime_type.contains("webp") {
        "webp"
    } else {
        "jpg"
    };

    let tmp_dir = std::env::temp_dir().join("tagmaster_dj_ffmpeg_artwork");
    fs::create_dir_all(&tmp_dir)
        .map_err(|err| FfmpegError::ExecutionFailed(format!("dossier temp impossible : {err}")))?;

    let input_path = tmp_dir.join(format!("art_in_{pid}_{nanos}.{in_ext}"));
    let output_path = tmp_dir.join(format!("art_out_{pid}_{nanos}.jpg"));

    fs::write(&input_path, &artwork.bytes)
        .map_err(|err| FfmpegError::ExecutionFailed(format!("écriture temp impossible : {err}")))?;

    let vf = format!(
        "crop=min(iw\\,ih):min(iw\\,ih),scale='min(iw,{max_size_px})':'min(ih,{max_size_px})'"
    );

    let output = Command::new(ffmpeg_bin)
        .args(["-y", "-v", "error", "-i"])
        .arg(&input_path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            &vf,
            "-c:v",
            "mjpeg",
            "-pix_fmt",
            "yuvj420p",
            "-q:v",
            "3",
        ])
        .arg(&output_path)
        .output();

    let _ = fs::remove_file(&input_path);

    let output = match output {
        Ok(out) => out,
        Err(err) => {
            let _ = fs::remove_file(&output_path);
            return Err(if err.kind() == std::io::ErrorKind::NotFound {
                FfmpegError::NotInstalled
            } else {
                FfmpegError::ExecutionFailed(err.to_string())
            });
        }
    };

    if !output.status.success() {
        let _ = fs::remove_file(&output_path);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(FfmpegError::ExecutionFailed(if stderr.is_empty() {
            format!("code de sortie {:?}", output.status.code())
        } else {
            stderr
        }));
    }

    let resized_bytes = fs::read(&output_path).map_err(|err| {
        FfmpegError::ExecutionFailed(format!("lecture pochette redimensionnée impossible : {err}"))
    })?;
    let _ = fs::remove_file(&output_path);

    if !is_baseline_jpeg(&resized_bytes) {
        return Err(FfmpegError::ExecutionFailed(
            "La pochette générée n'est pas un JPEG baseline valide.".to_string(),
        ));
    }

    Ok(DownloadedArtwork {
        bytes: resized_bytes,
        mime_type: "image/jpeg".to_string(),
    })
}

/// Prépare la pochette selon les règles du profil matériel (redimensionnement + JPEG baseline).
/// En cas d'absence de `ffmpeg`, retourne proprement la pochette d'origine si compatible, ou `None`.
pub fn prepare_artwork_for_profile(
    artwork: Option<&DownloadedArtwork>,
    profile: &HardwareProfile,
) -> Option<DownloadedArtwork> {
    let art = artwork?;
    if art.bytes.is_empty() {
        return None;
    }

    let max_px = max_artwork_size_for_profile(profile);
    let must_baseline = requires_baseline_jpeg(profile);
    let is_webp = art.mime_type.to_ascii_lowercase().contains("webp");

    if must_baseline || is_webp {
        match resize_artwork_to_baseline_jpeg(art, max_px) {
            Ok(processed) => return Some(processed),
            Err(_) if !must_baseline || is_baseline_jpeg(&art.bytes) => return Some(art.clone()),
            Err(_) => return None,
        }
    }

    // Profil Standard : redimensionne en JPEG baseline <= 1000px si FFmpeg est présent, sinon conserve tel quel
    match resize_artwork_to_baseline_jpeg(art, max_px) {
        Ok(processed) => Some(processed),
        Err(_) => Some(art.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_transcode_and_artwork_rules_per_hardware_profile() {
        // Standard
        assert!(!should_transcode_to_mp3(
            &HardwareProfile::Standard,
            &ExportFormatMode::Passthrough,
            "flac"
        ));
        assert!(should_transcode_to_mp3(
            &HardwareProfile::Standard,
            &ExportFormatMode::ForceMp3,
            "flac"
        ));
        assert_eq!(max_artwork_size_for_profile(&HardwareProfile::Standard), 1000);
        assert!(!requires_baseline_jpeg(&HardwareProfile::Standard));

        // iPod : Passthrough MP3/M4A, transcode FLAC/WAV -> MP3 320k, pochette 600px baseline
        assert!(!should_transcode_to_mp3(
            &HardwareProfile::Ipod,
            &ExportFormatMode::Passthrough,
            "mp3"
        ));
        assert!(!should_transcode_to_mp3(
            &HardwareProfile::Ipod,
            &ExportFormatMode::Passthrough,
            "m4a"
        ));
        assert!(should_transcode_to_mp3(
            &HardwareProfile::Ipod,
            &ExportFormatMode::Passthrough,
            "flac"
        ));
        assert!(should_transcode_to_mp3(
            &HardwareProfile::Ipod,
            &ExportFormatMode::Passthrough,
            "wav"
        ));
        assert_eq!(max_artwork_size_for_profile(&HardwareProfile::Ipod), 600);
        assert!(requires_baseline_jpeg(&HardwareProfile::Ipod));

        // PSP : Force MP3 320k toujours, pochette 400px baseline
        assert!(should_transcode_to_mp3(
            &HardwareProfile::Psp,
            &ExportFormatMode::Passthrough,
            "mp3"
        ));
        assert!(should_transcode_to_mp3(
            &HardwareProfile::Psp,
            &ExportFormatMode::Passthrough,
            "m4a"
        ));
        assert_eq!(max_artwork_size_for_profile(&HardwareProfile::Psp), 400);
        assert!(requires_baseline_jpeg(&HardwareProfile::Psp));
    }

    #[test]
    fn detects_baseline_jpeg_vs_progressive_jpeg_and_non_jpeg() {
        // Minimal baseline JPEG (SOI + SOF0 + EOI)
        let baseline = [
            0xFF, 0xD8, // SOI
            0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00, // SOF0
            0xFF, 0xD9, // EOI
        ];
        assert!(is_baseline_jpeg(&baseline));

        // Minimal progressive JPEG (SOI + SOF2 + EOI)
        let progressive = [
            0xFF, 0xD8, // SOI
            0xFF, 0xC2, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00, // SOF2
            0xFF, 0xD9, // EOI
        ];
        assert!(!is_baseline_jpeg(&progressive));

        // PNG header is not JPEG
        assert!(!is_baseline_jpeg(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A]));
    }

    #[test]
    fn handles_missing_ffmpeg_binary_gracefully_without_crashing() {
        let missing = Path::new("/nonexistent/tagmaster_missing_ffmpeg_bin");
        let err = transcode_to_mp3_320_with_binary(
            missing,
            Path::new("/tmp/in.wav"),
            Path::new("/tmp/out.mp3"),
        )
        .expect_err("missing ffmpeg must return FfmpegError::NotInstalled");
        assert_eq!(err, FfmpegError::NotInstalled);
        assert!(err.to_string().contains("FFmpeg introuvable"));

        assert!(resolve_ffmpeg_from_candidates(&["/nonexistent/tagmaster_missing_ffmpeg"]).is_none());
    }

    #[test]
    fn resizes_image_to_baseline_jpeg_when_ffmpeg_is_installed() {
        let Some(ffmpeg_bin) = resolve_ffmpeg_binary() else {
            return;
        };

        // Génère un PNG synthétique 800x600 avec ffmpeg puis vérifie le redimensionnement 400px baseline JPEG
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let src_png = std::env::temp_dir().join(format!("tagmaster_art_src_{nanos}.png"));
        let gen_status = Command::new(&ffmpeg_bin)
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=800x600",
                "-frames:v",
                "1",
            ])
            .arg(&src_png)
            .status()
            .expect("generate synthetic png");
        assert!(gen_status.success());

        let png_bytes = fs::read(&src_png).expect("read synthetic png");
        let _ = fs::remove_file(&src_png);

        let art = DownloadedArtwork {
            bytes: png_bytes,
            mime_type: "image/png".to_string(),
        };

        let resized = resize_artwork_to_baseline_jpeg_with_binary(&ffmpeg_bin, &art, 400)
            .expect("resize to 400px baseline JPEG");
        assert_eq!(resized.mime_type, "image/jpeg");
        assert!(is_baseline_jpeg(&resized.bytes));
    }
}
