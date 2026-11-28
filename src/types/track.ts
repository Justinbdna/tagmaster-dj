export type ExportMode = "flat" | "dynamic";

export type ExportFormatMode = "passthrough" | "force_mp3";

export type HardwareProfile = "standard" | "ipod" | "psp";

export type ThemeMode = "dark" | "light";

export type GlassTint = "clear" | "tinted";

export type AppView = "library" | "review" | "duplicates" | "settings";

export type TrackStatus =
  | "pending"
  | "processing"
  | "completed"
  | "needs_review"
  | "failed";

export type MetadataSource =
  | "apple_music"
  | "deezer"
  | "music_brainz"
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
  lyrics?: string;
  syncedLyrics?: string;
  bpm?: number;
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
  localArtworkPath?: string;
  embeddedArtworkPath?: string;
  reviewReason?: string;
  candidates?: TrackCandidate[];
  lastQueryUsed?: string;
  durationSecs?: number;
  bitrateKbps?: number;
  sampleRateHz?: number;
}

export interface ScanResult {
  tracks: AudioTrackItem[];
  skippedCount: number;
  scannedFolders: string[];
}

export interface ExistingTrackTags {
  title?: string;
  artist?: string;
  albumArtist?: string;
  album?: string;
  tagCount: number;
  hasDirtyTags: boolean;
  embeddedArtworkPath?: string;
  bpm?: number;
}

export type LookupStatus = "matched" | "protected_version" | "not_found";

export interface TrackCandidate {
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
  storefront: string;
  confidenceScore: number;
  source?: MetadataSource;
  platform?: string;
  durationSecs?: number;
  bpm?: number;
}

export type QuickSearchSource =
  | "auto"
  | "apple_music"
  | "deezer"
  | "youtube"
  | "soundcloud"
  | "music_brainz"
  | "discogs";

export interface ArtworkCandidate {
  artworkUrl: string;
  previewUrl: string;
  title: string;
  artist: string;
  album: string;
  year?: number;
  storefront: string;
}

export interface ApiKeysSettings {
  geniusApiKey: string;
  hasGeniusApiKey: boolean;
  discogsToken: string;
  hasDiscogsToken: boolean;
  envFilePath: string;
}

export interface MetadataLookupResult {
  status: LookupStatus;
  queryUsed: string;
  storefront?: string;
  metadata?: TrackMetadata;
  candidates?: TrackCandidate[];
  reason?: string;
  detectedKeyword?: string;
}

export interface BatchTrackInput {
  id: string;
  sourcePath: string;
  cachedMetadata?: TrackMetadata;
  localArtworkPath?: string;
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
  localArtworkPath?: string;
  embeddedArtworkPath?: string;
  reviewReason?: string;
  queryUsed?: string;
  candidates?: TrackCandidate[];
}

export interface ManualExportInput {
  sourcePath: string;
  exportMode: ExportMode;
  formatMode?: ExportFormatMode;
  hardwareProfile?: HardwareProfile;
  exportDir?: string;
  title: string;
  artist: string;
  albumArtist?: string;
  album?: string;
  year?: number;
  genre?: string;
  bpm?: number;
  trackNumber?: number;
  trackTotal?: number;
  discNumber?: number;
  discTotal?: number;
  isCompilation?: boolean;
  artworkUrl?: string;
  localArtworkPath?: string;
  source?: MetadataSource;
}

export interface ManualExportResult {
  metadata: TrackMetadata;
  exportedPath: string;
}

export interface BatchSummary {
  total: number;
  completedCount: number;
  needsReviewCount: number;
  failedCount: number;
  exportDirectory: string;
  interruptedByNetwork?: boolean;
  networkErrorMessage?: string;
  remainingCount?: number;
  lastExportedPath?: string;
}

export interface ArtworkExtractInput {
  id: string;
  sourcePath: string;
}

export interface TrackEmbeddedArtwork {
  trackId: string;
  embeddedArtworkPath: string;
}

export interface SessionCache {
  version: number;
  updatedAtMs: number;
  exportMode: ExportMode;
  exportDir?: string;
  tracks: AudioTrackItem[];
}

export interface SessionCacheSummary {
  hasSession: boolean;
  isUnfinished: boolean;
  updatedAtMs: number;
  exportMode: ExportMode;
  totalTracks: number;
  exportedCount: number;
  identifiedCount: number;
  needsReviewCount: number;
  remainingCount: number;
  cacheFilePath: string;
}

export interface DuplicateGroup {
  groupKey: string;
  displayLabel: string;
  matchReason: string;
  trackIds: string[];
}

export interface NetworkInterruptionState {
  action: "lookup" | "export";
  errorMessage: string;
  remainingCount: number;
  readyToExportCount: number;
  exportedCount: number;
}

export interface QueueStats {
  totalCount: number;
  pendingCount: number;
  identifiedCount: number;
  exportedCount: number;
  needsReviewCount: number;
  failedCount: number;
  processingCount: number;
  identifiableCount: number;
  exportableCount: number;
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

export interface FfmpegStatus {
  available: boolean;
  version?: string;
  binaryPath?: string;
}

export interface BinaryDependencyStatus {
  name: string;
  available: boolean;
  version?: string;
  binaryPath?: string;
}

export interface RuntimeDependenciesStatus {
  platform: string;
  ffmpeg: BinaryDependencyStatus;
  ytdlp: BinaryDependencyStatus;
  allAvailable: boolean;
  installCommand?: string;
}
