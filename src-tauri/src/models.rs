use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportMode {
    Flat,
    Dynamic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormatMode {
    Passthrough,
    ForceMp3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HardwareProfile {
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
    YtDlp,
    Genius,
    Discogs,
    Manual,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub review_reason: Option<String>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LookupStatus {
    Matched,
    ProtectedVersion,
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataLookupResult {
    pub status: LookupStatus,
    pub query_used: String,
    pub storefront: Option<String>,
    pub metadata: Option<TrackMetadata>,
    pub reason: Option<String>,
    pub detected_keyword: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTrackInput {
    pub id: String,
    pub source_path: String,
    pub cached_metadata: Option<TrackMetadata>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummary {
    pub total: usize,
    pub completed_count: usize,
    pub needs_review_count: usize,
    pub failed_count: usize,
    pub export_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub app_name: String,
    pub version: String,
    pub status: String,
}
