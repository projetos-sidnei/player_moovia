// Nomes dos eventos Tauri (listen) — espelham constants.rs do backend.
export const EVENTS = {
  PLAYBACK_PROGRESS: "playback-progress",
  PLAYBACK_STOPPED: "playback-stopped",
  PLAYBACK_ENDED: "playback-ended",
  PLAYER_CLOSED: "player-closed",
  THUMBNAIL_PROGRESS: "thumbnail-progress",
  INTRO_ANALYSIS_PROGRESS: "intro-analysis-progress",
} as const;
