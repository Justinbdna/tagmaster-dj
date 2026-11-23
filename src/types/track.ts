export type ExportMode = "flat" | "dynamic";

export type ExportFormatMode = "passthrough" | "force_mp3";

export type HardwareProfile = "standard" | "ipod" | "psp";

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
  status: TrackStatus;
  metadata?: TrackMetadata;
  reviewReason?: string;
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
