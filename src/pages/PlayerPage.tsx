// Raiz da janela sobreposta do player: fundo transparente, controles flutuando
// sobre a janela nativa onde o libVLC desenha o vídeo (ver vlc_engine/host.rs).

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { EVENTS } from "../constants/events.constants";
import { AUTO_NEXT_SECONDS, CONTROLS_HIDE_MS, DELAY_STEP_MS, PLAYBACK_RATES, PREVIEW_BUCKET_SECONDS, PREVIEW_CACHE_MAX_ENTRIES, PREVIEW_DEBOUNCE_MS, SEEK_STEP_SECONDS, VOLUME_DEFAULT, VOLUME_MAX } from "../constants/player.constants";
import type { IntroMarker, ProgressEventPayload, TrackMenu, VideoWithProgress } from "../types";
import { useSettingsStore } from "../store/settings";
import { formatTime } from "../utils/time";

export function PlayerPage({ videoId, collectionId }: { videoId: number; collectionId: number }) {
  const [currentVideoId, setCurrentVideoId] = useState(videoId);
  const [position, setPosition] = useState(0);
  const [duration, setDuration] = useState<number | null>(null);
  const [paused, setPaused] = useState(false);
  const [volume, setVolumeState] = useState(VOLUME_DEFAULT);
  const [marker, setMarker] = useState<IntroMarker | null>(null);
  const [episodes, setEpisodes] = useState<VideoWithProgress[]>([]);
  // Abertura só faz sentido em série: filme não marca nem pula intro.
  const [isMovie, setIsMovie] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [markStart, setMarkStart] = useState<number | null>(null);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [fullscreen, setFullscreen] = useState(false);
  const [trackMenu, setTrackMenu] = useState<TrackMenu | null>(null);
  const [showTracks, setShowTracks] = useState(false);
  const [showPlaylist, setShowPlaylist] = useState(false);
  const [rate, setRate] = useState(1);
  const [audioDelay, setAudioDelay] = useState(0);
  const [subtitleDelay, setSubtitleDelay] = useState(0);
  // Contagem regressiva mostrada ao fim do episódio.
  const [autoNext, setAutoNext] = useState<number | null>(null);
  const [hover, setHover] = useState<{ x: number; time: number } | null>(null);
  const [hoverFrame, setHoverFrame] = useState<string | null>(null);
  const previewCache = useRef(new Map<number, string>());
  const previewRequests = useRef(new Set<number>());
  const previewGeneration = useRef(0);
  const previewTimerRef = useRef<number | undefined>(undefined);
  const hoverBucketRef = useRef<number | null>(null);
  const { autoSkipIntro } = useSettingsStore();
  const seekingRef = useRef(false);
  const hideTimerRef = useRef<number | undefined>(undefined);
  const clickTimerRef = useRef<number | undefined>(undefined);
  const volumeBeforeMuteRef = useRef(VOLUME_DEFAULT);
  const fullscreenRef = useRef(fullscreen);
  fullscreenRef.current = fullscreen;
  const positionRef = useRef(position);
  positionRef.current = position;

  // Pausado ou com um painel aberto, os controles ficam fixos.
  const keepVisibleRef = useRef(false);
  keepVisibleRef.current = paused || showTracks || showPlaylist;

  const pokeControls = useCallback(() => {
    setControlsVisible(true);
    window.clearTimeout(hideTimerRef.current);
    if (!keepVisibleRef.current) {
      hideTimerRef.current = window.setTimeout(() => setControlsVisible(false), CONTROLS_HIDE_MS);
    }
  }, []);

  useEffect(() => {
    pokeControls();
    return () => window.clearTimeout(hideTimerRef.current);
  }, [paused, showTracks, showPlaylist, pokeControls]);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    api
      .playVideo(currentVideoId)
      .then((info) => {
        if (cancelled) return;
        setPosition(info.startAtSeconds);
        setDuration(info.durationSeconds);
        setMarker(info.introMarker);
        setPaused(false);
        api.setVolume(volume).catch(() => {});
      })
      .catch((e) => setError(String(e)));
    return () => {
      cancelled = true;
    };
    // volume intencionalmente fora das deps: só re-aplicado na troca de vídeo
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentVideoId]);

  useEffect(() => {
    api
      .getCollectionDetail(collectionId)
      .then((d) => {
        setEpisodes([...d.seasons.flatMap((s) => s.videos), ...d.looseVideos]);
        setIsMovie(d.collectionType === "movie");
      })
      .catch(() => {});
  }, [collectionId]);

  useEffect(() => {
    const unlisten = listen<ProgressEventPayload>(EVENTS.PLAYBACK_PROGRESS, (event) => {
      if (event.payload.videoId !== currentVideoId || seekingRef.current) return;
      setPosition(event.payload.positionSeconds);
      if (event.payload.durationSeconds != null) setDuration(event.payload.durationSeconds);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [currentVideoId]);

  // Ao trocar de vídeo, a sincronia e a contagem regressiva voltam ao normal
  // (a velocidade escolhida é mantida, como nos players de streaming).
  useEffect(() => {
    setAutoNext(null);
    setAudioDelay(0);
    setSubtitleDelay(0);
    api.setPlaybackRate(rate).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentVideoId]);

  // Fechar o player é do backend: ele para o VLC, esconde o vídeo, some com esta
  // janela e avisa a biblioteca para recarregar o progresso.
  const closePlayer = useCallback(() => {
    api.closePlayer().catch(console.error);
  }, []);

  const toggleFullscreen = useCallback(() => {
    const next = !fullscreenRef.current;
    api
      .setPlayerFullscreen(next)
      .then(() => setFullscreen(next))
      .catch(() => {});
  }, []);

  // Prévia da barra: o frame vem do backend (segundo player, sem tocar na
  // reprodução). Debounce + cache por faixa de tempo para não decodificar à toa.
  const onSeekHover = useCallback(
    (e: React.MouseEvent<HTMLInputElement>) => {
      if (!duration) return;
      const rect = e.currentTarget.getBoundingClientRect();
      const ratio = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
      const time = ratio * duration;
      const bucket = Math.max(0, Math.round(time / PREVIEW_BUCKET_SECONDS) * PREVIEW_BUCKET_SECONDS);
      setHover({ x: e.clientX - rect.left, time });

      if (hoverBucketRef.current === bucket) return;
      hoverBucketRef.current = bucket;

      const cached = previewCache.current.get(bucket);
      setHoverFrame(cached ?? null);
      if (cached) return;
      if (previewRequests.current.has(bucket)) return;

      window.clearTimeout(previewTimerRef.current);
      const requestGeneration = previewGeneration.current;
      previewRequests.current.add(bucket);
      previewTimerRef.current = window.setTimeout(() => {
        api
          .getPreviewFrame(bucket)
          .then((uri) => {
            if (!uri || requestGeneration !== previewGeneration.current) return;
            if (previewCache.current.size >= PREVIEW_CACHE_MAX_ENTRIES) {
              const oldest = previewCache.current.keys().next().value;
              if (oldest !== undefined) previewCache.current.delete(oldest);
            }
            previewCache.current.set(bucket, uri);
            // O mouse pode ter andado enquanto o frame era decodificado.
            if (hoverBucketRef.current === bucket) setHoverFrame(uri);
          })
          .catch(() => {})
          .finally(() => previewRequests.current.delete(bucket));
      }, PREVIEW_DEBOUNCE_MS);
    },
    [duration],
  );

  const clearSeekHover = useCallback(() => {
    window.clearTimeout(previewTimerRef.current);
    hoverBucketRef.current = null;
    setHover(null);
    setHoverFrame(null);
  }, []);

  // Cada vídeo tem seus próprios frames.
  useEffect(() => {
    previewCache.current.clear();
    previewRequests.current.clear();
    previewGeneration.current += 1;
    window.clearTimeout(previewTimerRef.current);
    hoverBucketRef.current = null;
  }, [currentVideoId]);

  const openTracks = useCallback(() => {
    setShowTracks((open) => {
      if (!open) {
        api
          .getTracks()
          .then(setTrackMenu)
          .catch(() => {});
      }
      return !open;
    });
  }, []);

  async function pickAudioTrack(id: number) {
    await api.setAudioTrack(id).catch(() => {});
    setTrackMenu((m) => (m ? { ...m, audioCurrent: id } : m));
  }

  async function pickSubtitleTrack(id: number) {
    await api.setSubtitleTrack(id).catch(() => {});
    setTrackMenu((m) => (m ? { ...m, subtitleCurrent: id } : m));
  }

  function changeRate(next: number) {
    setRate(next);
    api.setPlaybackRate(next).catch(() => {});
  }

  function changeAudioDelay(next: number) {
    setAudioDelay(next);
    api.setAudioDelay(next).catch(() => {});
  }

  function changeSubtitleDelay(next: number) {
    setSubtitleDelay(next);
    api.setSubtitleDelay(next).catch(() => {});
  }

  const currentIndex = useMemo(() => episodes.findIndex((e) => e.id === currentVideoId), [episodes, currentVideoId]);
  const nextEpisode = currentIndex >= 0 ? episodes[currentIndex + 1] : undefined;
  const previousEpisode = currentIndex > 0 ? episodes[currentIndex - 1] : undefined;
  const currentEpisode = currentIndex >= 0 ? episodes[currentIndex] : undefined;
  const nextEpisodeRef = useRef<VideoWithProgress | undefined>(undefined);
  nextEpisodeRef.current = nextEpisode;

  // Fim do episódio: emenda o próximo com contagem regressiva (dá para assistir
  // na hora ou cancelar). Sem próximo episódio, o player fecha.
  useEffect(() => {
    const unlisten = listen<number>(EVENTS.PLAYBACK_ENDED, (event) => {
      if (event.payload !== currentVideoId) return;
      if (!nextEpisodeRef.current) {
        closePlayer();
        return;
      }
      setPaused(true);
      setAutoNext(AUTO_NEXT_SECONDS);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [currentVideoId, closePlayer]);

  useEffect(() => {
    if (autoNext === null) return;
    if (autoNext <= 0) {
      const next = nextEpisodeRef.current;
      setAutoNext(null);
      if (next) setCurrentVideoId(next.id);
      return;
    }
    const timer = window.setTimeout(() => setAutoNext((s) => (s === null ? null : s - 1)), 1000);
    return () => window.clearTimeout(timer);
  }, [autoNext]);

  const doSeek = useCallback((seconds: number) => {
    seekingRef.current = true;
    setPosition(seconds);
    api
      .seek(seconds)
      .catch(() => {})
      .finally(() => {
        window.setTimeout(() => {
          seekingRef.current = false;
        }, 500);
      });
  }, []);

  const togglePause = useCallback(async () => {
    await api.togglePause().catch(() => {});
    setPaused((p) => !p);
  }, []);

  function changeVolume(v: number) {
    setVolumeState(v);
    volumeBeforeMuteRef.current = v > 0 ? v : volumeBeforeMuteRef.current;
    api.setVolume(v).catch(() => {});
  }

  // Mudo = volume 0 guardando o valor anterior. Não usa o mute do libVLC, que
  // no Windows muta a sessão de áudio do processo inteiro (ver decisoes.md).
  const toggleMute = useCallback(() => {
    setVolumeState((current) => {
      const next = current > 0 ? 0 : volumeBeforeMuteRef.current || VOLUME_DEFAULT;
      if (current > 0) volumeBeforeMuteRef.current = current;
      api.setVolume(next).catch(() => {});
      return next;
    });
  }, []);

  // Atalhos: espaço = pausa · setas = ±10s · F = tela cheia · Esc = sair
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement) return;
      pokeControls();
      if (e.code === "Space") {
        e.preventDefault();
        togglePause();
      } else if (e.code === "ArrowLeft") {
        doSeek(Math.max(0, positionRef.current - SEEK_STEP_SECONDS));
      } else if (e.code === "ArrowRight") {
        doSeek(positionRef.current + SEEK_STEP_SECONDS);
      } else if (e.code === "KeyF") {
        toggleFullscreen();
      } else if (e.code === "KeyM") {
        toggleMute();
      } else if (e.code === "Escape") {
        if (fullscreenRef.current) {
          toggleFullscreen();
        } else {
          closePlayer();
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [togglePause, doSeek, closePlayer, pokeControls, toggleFullscreen, toggleMute]);

  async function markIntro() {
    if (markStart === null) {
      setMarkStart(position);
      return;
    }
    const seasonId = currentEpisode?.seasonId ?? null;
    try {
      await api.setIntroMarker(collectionId, seasonId, markStart, position);
      setMarker({ startSeconds: markStart, endSeconds: position });
    } catch (e) {
      setError(String(e));
    } finally {
      setMarkStart(null);
    }
  }

  const showSkipIntro = !isMovie && marker !== null && !autoSkipIntro && position >= marker.startSeconds && position < marker.endSeconds;

  return (
    <div className={`player-screen ${controlsVisible ? "" : "controls-hidden"}`} onMouseMove={pokeControls}>
      <header className="player-top player-fade">
        <button className="ghost" onClick={closePlayer}>
          ← Voltar
        </button>
        <span className="player-title">{currentEpisode?.displayName ?? ""}</span>
      </header>

      {/* Área central: clique pausa/retoma, duplo clique = tela cheia */}
      <div
        className="player-center"
        onClick={() => {
          window.clearTimeout(clickTimerRef.current);
          clickTimerRef.current = window.setTimeout(togglePause, 250);
        }}
        onDoubleClick={() => {
          window.clearTimeout(clickTimerRef.current);
          toggleFullscreen();
        }}
      >
        <div className="center-controls player-fade" onClick={(e) => e.stopPropagation()}>
          <button className="circle" onClick={() => previousEpisode && setCurrentVideoId(previousEpisode.id)} disabled={!previousEpisode} title="Episódio anterior">
            ⏮
          </button>
          <button className="circle" onClick={() => doSeek(Math.max(0, position - SEEK_STEP_SECONDS))} title={`Voltar ${SEEK_STEP_SECONDS}s (←)`}>
            ↺<small>{SEEK_STEP_SECONDS}</small>
          </button>
          <button className="circle big" onClick={togglePause} title="Pausar/retomar (espaço)">
            {paused ? "▶" : "❚❚"}
          </button>
          <button className="circle" onClick={() => doSeek(position + SEEK_STEP_SECONDS)} title={`Avançar ${SEEK_STEP_SECONDS}s (→)`}>
            ↻<small>{SEEK_STEP_SECONDS}</small>
          </button>
          <button className="circle" onClick={() => nextEpisode && setCurrentVideoId(nextEpisode.id)} disabled={!nextEpisode} title="Próximo episódio">
            ⏭
          </button>
        </div>
      </div>

      {error && <p className="status player-error">{error}</p>}

      {showSkipIntro && marker && (
        <button className="skip-intro" onClick={() => doSeek(marker.endSeconds)}>
          Pular abertura ⏭
        </button>
      )}

      {autoNext !== null && nextEpisode && (
        <div className="auto-next">
          <span className="auto-next-label">A seguir</span>
          <strong>{nextEpisode.displayName}</strong>
          <div className="auto-next-actions">
            <button className="primary" onClick={() => setCurrentVideoId(nextEpisode.id)}>
              Assistir agora ({autoNext}s)
            </button>
            <button onClick={() => setAutoNext(null)}>Cancelar</button>
          </div>
        </div>
      )}

      {showPlaylist && (
        <aside className="playlist-panel player-fade" onClick={(e) => e.stopPropagation()}>
          <header className="playlist-head">
            <h3>Episódios</h3>
            <button className="ghost" onClick={() => setShowPlaylist(false)} title="Fechar">
              ✕
            </button>
          </header>
          <div className="playlist-items">
            {episodes.map((ep) => (
              <button key={ep.id} className={`playlist-item ${ep.id === currentVideoId ? "current" : ""}`} onClick={() => setCurrentVideoId(ep.id)}>
                <span className={`episode-status ${ep.watched ? "watched" : ep.positionSeconds > 0 ? "in-progress" : ""}`}>{ep.id === currentVideoId ? "▶" : ep.watched ? "✓" : "•"}</span>
                <span className="playlist-name">{ep.displayName}</span>
                {ep.durationSeconds && <span className="playlist-time">{formatTime(ep.durationSeconds)}</span>}
              </button>
            ))}
          </div>
        </aside>
      )}

      {showTracks && trackMenu && (
        <div className="tracks-panel player-fade">
          <div className="tracks-col">
            <h3>Áudio</h3>
            <div className="track-list">
              {trackMenu.audio.length === 0 && <span className="tracks-empty">Nenhuma trilha</span>}
              {trackMenu.audio.map((t) => (
                <button key={t.id} className={`track-option ${t.id === trackMenu.audioCurrent ? "active" : ""}`} onClick={() => pickAudioTrack(t.id)}>
                  {t.name || `Trilha ${t.id}`}
                </button>
              ))}
            </div>
          </div>
          <div className="tracks-col">
            <h3>Sincronia</h3>
            <div className="sync-row">
              <span>Áudio</span>
              <button onClick={() => changeAudioDelay(audioDelay - DELAY_STEP_MS)}>−</button>
              <span className="sync-value">{audioDelay} ms</span>
              <button onClick={() => changeAudioDelay(audioDelay + DELAY_STEP_MS)}>+</button>
              <button onClick={() => changeAudioDelay(0)} title="Zerar">
                ↺
              </button>
            </div>
            <div className="sync-row">
              <span>Legenda</span>
              <button onClick={() => changeSubtitleDelay(subtitleDelay - DELAY_STEP_MS)}>−</button>
              <span className="sync-value">{subtitleDelay} ms</span>
              <button onClick={() => changeSubtitleDelay(subtitleDelay + DELAY_STEP_MS)}>+</button>
              <button onClick={() => changeSubtitleDelay(0)} title="Zerar">
                ↺
              </button>
            </div>
            <p className="hint sync-hint">Positivo atrasa em relação ao vídeo.</p>
          </div>
          <div className="tracks-col">
            <h3>Legendas ({trackMenu.subtitles.length})</h3>
            <div className="track-list">
              {trackMenu.subtitles.length === 0 && <span className="tracks-empty">Nenhuma legenda</span>}
              {trackMenu.subtitles.map((t) => (
                <button key={t.id} className={`track-option ${t.id === trackMenu.subtitleCurrent ? "active" : ""}`} onClick={() => pickSubtitleTrack(t.id)}>
                  {t.name || `Legenda ${t.id}`}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      <footer className="player-controls player-fade">
        <div className="seek-row">
          <span className="time-label">{formatTime(position)}</span>
          <div className="seek-wrap">
            <input className="seek-bar" type="range" min={0} max={duration ?? Math.max(position, 1)} step={1} value={position} onChange={(e) => doSeek(Number(e.target.value))} onMouseMove={onSeekHover} onMouseLeave={clearSeekHover} />
            {hover && (
              <div className="seek-preview" style={{ left: `${hover.x}px` }}>
                {hoverFrame ? <img src={hoverFrame} alt="" /> : <div className="seek-preview-placeholder" />}
                <span>{formatTime(hover.time)}</span>
              </div>
            )}
          </div>
          <span className="time-label">{duration ? formatTime(duration) : "--:--"}</span>
        </div>
        <div className="controls-row">
          <div className="controls-left">
            <div className="volume">
              <button className="volume-toggle" onClick={toggleMute} title={volume === 0 ? "Ativar som (M)" : "Mudo (M)"}>
                {volume === 0 ? "🔇" : "🔊"}
              </button>
              <input type="range" min={0} max={VOLUME_MAX} value={volume} onChange={(e) => changeVolume(Number(e.target.value))} title="Volume" />
            </div>
            {!isMovie && (
              <button className={`ghost ${markStart !== null ? "toggled" : ""}`} onClick={markIntro} title="Marcar o intervalo da abertura para pular nos próximos episódios">
                {markStart === null ? "◧ Marcar abertura" : `◨ Fim da abertura (início: ${formatTime(markStart)})`}
              </button>
            )}
          </div>
          <div className="controls-right">
            <select className="rate-select" value={rate} onChange={(e) => changeRate(Number(e.target.value))} title="Velocidade de reprodução">
              {PLAYBACK_RATES.map((r) => (
                <option key={r} value={r}>
                  {r}×
                </option>
              ))}
            </select>
            <button className={`ghost ${showTracks ? "toggled" : ""}`} onClick={openTracks} title="Áudio e legendas">
              🗨 Áudio/Legenda
            </button>
            <button className={`ghost ${showPlaylist ? "toggled" : ""}`} onClick={() => setShowPlaylist((open) => !open)} title="Lista de episódios">
              ☰ Episódios
            </button>
            <button className="ghost" onClick={toggleFullscreen} title="Tela cheia (F ou duplo clique)">
              {fullscreen ? "🗗" : "⛶"}
            </button>
          </div>
        </div>
      </footer>
    </div>
  );
}
