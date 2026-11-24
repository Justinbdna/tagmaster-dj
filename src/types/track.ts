export type ExportMode = "flat" | "dynamic";

export type ExportFormatMode = "passthrough" | "force_mp3";

export type HardwareProfile = "standard" | "ipod" | "psp";

export type ThemeMode = "dark" | "light";

export type GlassTint = "clear" | "tinted";

export type AppView = "library" | "review" | "settings";

export type TrackStatus =
  | "pending"
  | "processing"
  | "completed"
  | "needs_review"
  | "failed";

export type MetadataSource =
  | "apple_music"
  | "yt_dlp"
  | "genius"
  | "discogs"
  | "manual";

export interface TrackMetadata {
  title: string;
  artist: string;
  albumArtist: string;
  album: string;
  trackNumber?: number;
  trackTotal?: number;
  discNumber?: number;
  discTotal?: number;
  year?: number;
  genre?: string;
  artworkUrl?: string;
  isCompilation: boolean;
  source?: MetadataSource;
}

export interface AudioTrackItem {
  id: string;
  sourcePath: string;
  fileName: string;
  parentFolder: string;
  extension: string;
  sizeBytes: number;
  hasProtectedKeyword: boolean;
  status: TrackStatus;
  metadata?: TrackMetadata;
  exportedPath?: string;
  reviewReason?: string;
}

export interface ScanResult {
  tracks: AudioTrackItem[];
  skippedCount: number;
  scannedFolders: string[];
}

export interface ExistingTrackTags {
  title?: string;
  artist?: string;
  album?: string;
  tagCount: number;
  hasDirtyTags: boolean;
}

export type LookupStatus = "matched" | "protected_version" | "not_found";

export interface MetadataLookupResult {
  status: LookupStatus;
  queryUsed: string;
  storefront?: string;
  metadata?: TrackMetadata;
  reason?: string;
  detectedKeyword?: string;
}

export interface BatchTrackInput {
  id: string;
  sourcePath: string;
  cachedMetadata?: TrackMetadata;
}

export interface PipelineProgressEvent {
  trackId: string;
  currentIndex: number;
  totalTracks: number;
  percent: number;
  currentFileName: string;
  trackStatus: TrackStatus;
  metadata?: TrackMetadata;
  exportedPath?: string;
  reviewReason?: string;
}

export interface BatchSummary {
  total: number;
  completedCount: number;
  needsReviewCount: number;
  failedCount: number;
  exportDirectory: string;
}

export interface PipelineConfig {
  exportMode: ExportMode;
  formatMode: ExportFormatMode;
  hardwareProfile: HardwareProfile;
  outputDirectory: string;
}

export interface HealthStatus {
  appName: string;
  version: string;
  status: string;
}
