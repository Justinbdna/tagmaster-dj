use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportMode {
    Flat,
    Dynamic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormatMode {
    #[default]
    Passthrough,
    ForceMp3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HardwareProfile {
    #[default]
    Standard,
    Ipod,
    Psp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrackStatus {
    Pending,
    Processing,
    Completed,
    NeedsReview,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataSource {
    AppleMusic,
    Deezer,
    MusicBrainz,
    YtDlp,
    Genius,
    Discogs,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchSource {
    Auto,
    AppleMusic,
    Deezer,
    Youtube,
    Soundcloud,
    MusicBrainz,
    Discogs,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub artwork_url: Option<String>,
    pub is_compilation: bool,
    pub source: Option<MetadataSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_lyrics: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrackItem {
    pub id: String,
    pub source_path: String,
    pub file_name: String,
    pub parent_folder: String,
    pub extension: String,
    pub size_bytes: u64,
    pub has_protected_keyword: bool,
    pub status: TrackStatus,
    pub metadata: Option<TrackMetadata>,
    pub exported_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_artwork_path: Option<String>,
    pub review_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<TrackCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_query_used: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub tracks: Vec<AudioTrackItem>,
    pub skipped_count: usize,
    pub scanned_folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExistingTrackTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub tag_count: usize,
    pub has_dirty_tags: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded_artwork_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LookupStatus {
    Matched,
    ProtectedVersion,
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrackCandidate {
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub artwork_url: Option<String>,
    pub is_compilation: bool,
    pub storefront: String,
    pub confidence_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<MetadataSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataLookupResult {
    pub status: LookupStatus,
    pub query_used: String,
    pub storefront: Option<String>,
    pub metadata: Option<TrackMetadata>,
    #[serde(default)]
    pub candidates: Vec<TrackCandidate>,
    pub reason: Option<String>,
    pub detected_keyword: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTrackInput {
    pub id: String,
    pub source_path: String,
    pub cached_metadata: Option<TrackMetadata>,
    #[serde(default)]
    pub local_artwork_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineProgressEvent {
    pub track_id: String,
    pub current_index: usize,
    pub total_tracks: usize,
    pub percent: u8,
    pub current_file_name: String,
    pub track_status: TrackStatus,
    pub metadata: Option<TrackMetadata>,
    pub exported_path: Option<String>,
    pub review_reason: Option<String>,
    pub query_used: Option<String>,
    #[serde(default)]
    pub candidates: Vec<TrackCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualExportInput {
    pub source_path: String,
    pub export_mode: ExportMode,
    #[serde(default)]
    pub format_mode: ExportFormatMode,
    #[serde(default)]
    pub hardware_profile: HardwareProfile,
    pub title: String,
    pub artist: String,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub is_compilation: Option<bool>,
    pub artwork_url: Option<String>,
    pub local_artwork_path: Option<String>,
    pub source: Option<MetadataSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualExportResult {
    pub metadata: TrackMetadata,
    pub exported_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummary {
    pub total: usize,
    pub completed_count: usize,
    pub needs_review_count: usize,
    pub failed_count: usize,
    pub export_directory: String,
    #[serde(default)]
    pub interrupted_by_network: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_error_message: Option<String>,
    #[serde(default)]
    pub remaining_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionCache {
    pub version: u32,
    pub updated_at_ms: u64,
    pub export_mode: ExportMode,
    pub tracks: Vec<AudioTrackItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionCacheSummary {
    pub has_session: bool,
    pub is_unfinished: bool,
    pub updated_at_ms: u64,
    pub export_mode: ExportMode,
    pub total_tracks: usize,
    pub exported_count: usize,
    pub identified_count: usize,
    pub needs_review_count: usize,
    pub remaining_count: usize,
    pub cache_file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub group_key: String,
    pub display_label: String,
    pub match_reason: String,
    pub track_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkCandidate {
    pub artwork_url: String,
    pub preview_url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: Option<u32>,
    pub storefront: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeysSettings {
    pub genius_api_key: String,
    pub has_genius_api_key: bool,
    #[serde(default)]
    pub discogs_token: String,
    #[serde(default)]
    pub has_discogs_token: bool,
    pub env_file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub app_name: String,
    pub version: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegStatus {
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BinaryDependencyStatus {
    pub name: String,
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDependenciesStatus {
    pub platform: String,
    pub ffmpeg: BinaryDependencyStatus,
    pub ytdlp: BinaryDependencyStatus,
    pub all_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_command: Option<String>,
}
