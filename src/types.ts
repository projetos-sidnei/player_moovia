// Interfaces espelhando os DTOs do backend (serde camelCase) — spec §4.2.

export type CollectionType = "series" | "movie" | "course";

export interface CollectionCard {
  id: number;
  title: string;
  collectionType: CollectionType;
  hasPoster: boolean;
  totalVideos: number;
  watchedVideos: number;
  hasInProgress: boolean;
}

export interface RenamePlanItem {
  videoId: number;
  currentName: string;
  newName: string;
  conflict: boolean;
  unchanged: boolean;
}

export interface EpisodeNamePlanItem {
  videoId: number;
  currentName: string;
  newName: string;
  locked: boolean;
  unchanged: boolean;
}

export interface DeleteOutcome {
  /** Arquivos enviados para a Lixeira. */
  trashed: number;
  errors: string[];
}

export interface RenameOutcome {
  renamed: number;
  skipped: number;
  errors: string[];
}

export interface SeasonRow {
  id: number;
  collectionId: number;
  title: string;
  folderPath: string;
  seasonNumber: number | null;
}

export interface VideoWithProgress {
  id: number;
  collectionId: number;
  seasonId: number | null;
  filePath: string;
  displayName: string;
  displayNameLocked: boolean;
  episodeNumber: number | null;
  durationSeconds: number | null;
  positionSeconds: number;
  watched: boolean;
}

export interface SeasonWithVideos {
  season: SeasonRow;
  videos: VideoWithProgress[];
}

export interface CollectionDetail {
  id: number;
  title: string;
  collectionType: CollectionType;
  showThumbnails: boolean;
  seasons: SeasonWithVideos[];
  looseVideos: VideoWithProgress[];
}

export interface VideoRow {
  id: number;
  collectionId: number;
  seasonId: number | null;
  filePath: string;
  displayName: string;
  episodeNumber: number | null;
  durationSeconds: number | null;
}

export interface ContinueWatchingItem {
  videoId: number;
  displayName: string;
  durationSeconds: number | null;
  collectionId: number;
  collectionTitle: string;
  positionSeconds: number;
}

export interface IntroMarker {
  startSeconds: number;
  endSeconds: number;
}

export interface UserSettings {
  id: number;
  autoSkipIntro: boolean;
}

export interface LibraryRoot {
  id: number;
  path: string;
  createdAt: string;
}

export interface ScanSummary {
  collections: number;
  seasons: number;
  videos: number;
  removed: number;
}

export interface ThumbnailProgress {
  collectionId: number;
  processed: number;
  total: number;
  generated: number;
  failed: number;
  current: string;
  cancelled?: boolean;
}

export interface IntroSuggestion {
  videoId: number;
  displayName: string;
  startSeconds: number;
  endSeconds: number;
  confidence: number;
}

export interface IntroAnalysisProgress {
  collectionId: number;
  processed: number;
  total: number;
  current: string;
  videoId?: number;
  cancelled?: boolean;
}

export interface PlaybackStartInfo {
  videoId: number;
  startAtSeconds: number;
  durationSeconds: number | null;
  introMarker: IntroMarker | null;
}

export interface ProgressEventPayload {
  videoId: number;
  positionSeconds: number;
  durationSeconds: number | null;
}

export interface TrackInfo {
  id: number;
  name: string;
}

export interface TrackMenu {
  audio: TrackInfo[];
  audioCurrent: number;
  subtitles: TrackInfo[];
  subtitleCurrent: number;
}
