const SECONDS_PER_HOUR = 3600;
const SECONDS_PER_MINUTE = 60;

/** 5025 -> "1:23:45"; 754 -> "12:34" */
export function formatTime(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / SECONDS_PER_HOUR);
  const m = Math.floor((s % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  const sec = s % SECONDS_PER_MINUTE;
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  const ss = String(sec).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

/** Fração 0..1 do progresso, com guarda para duração nula/zero. */
export function progressRatio(position: number, duration: number | null): number {
  if (!duration || duration <= 0) return 0;
  return Math.min(1, position / duration);
}
