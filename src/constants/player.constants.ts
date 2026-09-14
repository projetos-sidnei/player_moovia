// Constantes da UI do player.
export const VOLUME_DEFAULT = 80;
export const VOLUME_MAX = 100;
export const SEEK_STEP_SECONDS = 10;
export const CONTROLS_HIDE_MS = 3000;

/** Label da janela sobreposta — espelha OVERLAY_WINDOW_LABEL do constants.rs. */
export const OVERLAY_WINDOW_LABEL = "player-overlay";

/** Espera o mouse "parar" antes de pedir o frame (decodificar custa ~300ms). */
export const PREVIEW_DEBOUNCE_MS = 180;

/** Granularidade da prévia: instantes são arredondados para cachear frames. */
export const PREVIEW_BUCKET_SECONDS = 5;

/** Limita a memória usada pelas imagens PNG da prévia em vídeos longos. */
export const PREVIEW_CACHE_MAX_ENTRIES = 120;

/** Contagem regressiva antes de emendar o próximo episódio. */
export const AUTO_NEXT_SECONDS = 10;

/** Velocidades oferecidas no player. */
export const PLAYBACK_RATES = [0.75, 1, 1.25, 1.5, 2] as const;

/** Passo dos ajustes de sincronia de áudio/legenda, em milissegundos. */
export const DELAY_STEP_MS = 100;
