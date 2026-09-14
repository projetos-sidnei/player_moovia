import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { EVENTS } from "../constants/events.constants";
import { OrganizePanel } from "../components/OrganizePanel";

import type { Navigator } from "../navigation";
import type { CollectionDetail, IntroAnalysisProgress, IntroSuggestion, SeasonWithVideos, VideoWithProgress } from "../types";
import { formatTime, progressRatio } from "../utils/time";

function VideoItem({ video, showThumbnails, onPlay, onRenamed }: { video: VideoWithProgress; showThumbnails: boolean; onPlay: () => void; onRenamed: (message: string | null) => void }) {
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [name, setName] = useState(video.displayName);
  const [thumb, setThumb] = useState<string | null>(null);
  const inProgress = !video.watched && video.positionSeconds > 0;

  // Só carrega miniatura já gerada (a geração é manual, no painel Organizar).
  useEffect(() => {
    if (!showThumbnails) {
      setThumb(null);
      return;
    }
    api
      .getEpisodeThumbnail(video.id)
      .then(setThumb)
      .catch(() => {});
  }, [showThumbnails, video.id]);

  async function processThumbnail() {
    try {
      const generated = await api.generateEpisodeThumbnail(video.id, true);
      if (generated) {
        const next = await api.getEpisodeThumbnail(video.id);
        setThumb(next);
        onRenamed(thumb ? "Miniatura reprocessada." : "Miniatura gerada.");
      } else {
        onRenamed("Não foi possível gerar a miniatura deste vídeo.");
      }
    } catch (e) {
      onRenamed(String(e));
    }
  }

  async function toggleWatched() {
    try {
      await api.setVideoWatched(video.id, !video.watched);
      onRenamed(video.watched ? "Marcado como não assistido (progresso zerado)." : "Marcado como assistido.");
    } catch (e) {
      onRenamed(String(e));
    }
  }

  async function save() {
    try {
      await api.setVideoDisplayName(video.id, name);
      setEditing(false);
      onRenamed(null);
    } catch (e) {
      onRenamed(String(e));
    }
  }

  async function removeFromLibrary() {
    try {
      await api.removeVideoFromLibrary(video.id);
      onRenamed("Episódio removido da biblioteca (o arquivo continua no disco).");
    } catch (e) {
      onRenamed(String(e));
    }
  }

  async function deleteFile() {
    try {
      const outcome = await api.deleteVideoFile(video.id);
      onRenamed(outcome.errors.length > 0 ? `Não foi possível excluir: ${outcome.errors.join("; ")}` : "Arquivo enviado para a Lixeira.");
    } catch (e) {
      onRenamed(String(e));
    }
  }

  if (confirmDelete) {
    return (
      <div className="episode confirming">
        <span className="confirm-text">
          Excluir <strong>{video.displayName}</strong>?
        </span>
        <button onClick={removeFromLibrary}>Só do player</button>
        <button className="danger" onClick={deleteFile}>
          Excluir do PC
        </button>
        <button onClick={() => setConfirmDelete(false)}>Cancelar</button>
      </div>
    );
  }

  if (editing) {
    return (
      <div className="episode editing">
        <input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") save();
            if (e.key === "Escape") setEditing(false);
          }}
        />
        <button title="Salvar o nome do episódio" onClick={save}>
          Salvar
        </button>
        <button
          title="Cancelar a edição do nome"
          onClick={() => {
            setName(video.displayName);
            setEditing(false);
          }}
        >
          Cancelar
        </button>
      </div>
    );
  }

  return (
    <div className="episode">
      <button className="episode-main" title="Reproduzir este episódio" onClick={onPlay}>
        <span className={`episode-status ${video.watched ? "watched" : inProgress ? "in-progress" : ""}`}>{video.watched ? "✓" : inProgress ? "▶" : "•"}</span>
        {thumb ? <img className="episode-thumb" src={thumb} alt="" /> : <span className="episode-thumb-empty" />}
        <span className="episode-name">{video.displayName}</span>
        <span className="episode-time">
          {inProgress && `${formatTime(video.positionSeconds)} / `}
          {video.durationSeconds ? formatTime(video.durationSeconds) : ""}
        </span>
        {inProgress && (
          <span className="progress-track episode-progress">
            <span className="progress-fill" style={{ width: `${progressRatio(video.positionSeconds, video.durationSeconds) * 100}%` }} />
          </span>
        )}
      </button>
      <button className="episode-edit" title={video.watched ? "Marcar como não assistido (zera o progresso)" : "Marcar como assistido"} onClick={toggleWatched}>
        {video.watched ? "↺" : "✓"}
      </button>
      <button className="episode-edit" title="Editar nome do episódio" onClick={() => setEditing(true)}>
        ✎
      </button>
      <button className="episode-edit" title={thumb ? "Reprocessar miniatura" : "Gerar miniatura"} onClick={processThumbnail}>
        🖼
      </button>
      <button className="episode-edit" title="Excluir episódio" onClick={() => setConfirmDelete(true)}>
        🗑
      </button>
    </div>
  );
}

function IntroMarkerForm({ collectionId, seasonId, onSaved }: { collectionId: number; seasonId: number | null; onSaved: (msg: string) => void }) {
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");

  async function save() {
    const startSeconds = Number(start);
    const endSeconds = Number(end);
    if (Number.isNaN(startSeconds) || Number.isNaN(endSeconds)) {
      onSaved("Informe início e fim da abertura em segundos.");
      return;
    }
    try {
      await api.setIntroMarker(collectionId, seasonId, startSeconds, endSeconds);
      onSaved("Abertura salva.");
    } catch (e) {
      onSaved(String(e));
    }
  }

  return (
    <span className="intro-form">
      <input placeholder="início (s)" value={start} onChange={(e) => setStart(e.target.value)} />
      <input placeholder="fim (s)" value={end} onChange={(e) => setEnd(e.target.value)} />
      <button title="Salvar o intervalo da abertura" onClick={save}>
        Salvar abertura
      </button>
    </span>
  );
}

function IntroAnalysisPanel({ collectionId, seasons, onStatus }: { collectionId: number; seasons: SeasonWithVideos[]; onStatus: (message: string) => void }) {
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [maxMinutes, setMaxMinutes] = useState("5");
  const [interval, setInterval] = useState("5");
  const [progress, setProgress] = useState<IntroAnalysisProgress | null>(null);
  const [suggestions, setSuggestions] = useState<IntroSuggestion[]>([]);
  const [running, setRunning] = useState(false);
  const [processedVideoIds, setProcessedVideoIds] = useState<Set<number>>(new Set());
  const [analysisMessage, setAnalysisMessage] = useState<string | null>(null);
  const analysisCancelRef = useRef(false);

  useEffect(() => {
    const unlisten = listen<IntroAnalysisProgress>(EVENTS.INTRO_ANALYSIS_PROGRESS, (event) => {
      if (event.payload.collectionId !== collectionId) return;
      setProgress(event.payload);
      if (event.payload.videoId != null && !event.payload.cancelled) {
        setProcessedVideoIds((current) => new Set(current).add(event.payload.videoId!));
      }
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [collectionId]);

  function toggleVideo(videoId: number) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(videoId)) next.delete(videoId);
      else next.add(videoId);
      return next;
    });
  }

  function toggleSeason(seasonVideos: VideoWithProgress[]) {
    const ids = seasonVideos.map((video) => video.id);
    setSelected((current) => {
      const next = new Set(current);
      const allSelected = ids.every((id) => next.has(id));
      ids.forEach((id) => (allSelected ? next.delete(id) : next.add(id)));
      return next;
    });
  }

  async function analyze() {
    if (selected.size < 2) {
      setAnalysisMessage("Selecione pelo menos dois episódios. Com apenas um, a análise não será processada.");
      onStatus("Selecione pelo menos dois episódios para comparar.");
      return;
    }
    setSuggestions([]);
    setProcessedVideoIds(new Set());
    setAnalysisMessage(null);
    analysisCancelRef.current = false;
    setRunning(true);
    try {
      const result = await api.analyzeIntro({
        collectionId,
        videoIds: [...selected],
        maxSeconds: Number(maxMinutes) * 60,
        sampleIntervalSeconds: Number(interval),
      });
      setSuggestions(result);
      if (!analysisCancelRef.current) {
        setAnalysisMessage(result.length > 0 ? `Análise concluída: ${result.length} sugestão(ões) encontrada(s).` : "Análise concluída: nenhum trecho repetido foi encontrado.");
        onStatus(result.length > 0 ? `${result.length} sugestão(ões) de introdução encontradas.` : "Nenhum trecho repetido foi encontrado.");
      }
    } catch (e) {
      onStatus(String(e));
    } finally {
      setRunning(false);
    }
  }

  async function cancel() {
    analysisCancelRef.current = true;
    await api.cancelIntroAnalysis().catch((e) => onStatus(String(e)));
    setAnalysisMessage("Interrupção solicitada. O episódio atual terminará antes de parar.");
  }

  async function accept(suggestion: IntroSuggestion) {
    try {
      await api.acceptIntroSuggestion(suggestion.videoId, suggestion.startSeconds, suggestion.endSeconds, suggestion.confidence);
      setSuggestions((current) => current.filter((item) => item.videoId !== suggestion.videoId));
      onStatus(`Sugestão aceita para ${suggestion.displayName}.`);
    } catch (e) {
      onStatus(String(e));
    }
  }

  return (
    <div className="intro-analysis">
      <h3>Detectar introdução</h3>
      <p className="hint">Analisa apenas episódios selecionados usando comparação de frames. O resultado é uma sugestão e não altera marcadores até você aceitar.</p>
      <div className="intro-analysis-options">
        <label>
          Primeiros minutos
          <input type="number" min="1" max="5" step="1" value={maxMinutes} onChange={(event) => setMaxMinutes(event.target.value)} disabled={running} />
        </label>
        <label>
          Intervalo (s)
          <input type="number" min="2" max="30" step="1" value={interval} onChange={(event) => setInterval(event.target.value)} disabled={running} />
        </label>
      </div>
      <div className="intro-analysis-selection">
        {seasons.map(({ season, videos: seasonVideos }) => {
          const allSelected = seasonVideos.length > 0 && seasonVideos.every((video) => selected.has(video.id));
          return (
            <fieldset key={season.id}>
              <legend>
                <label>
                  <input type="checkbox" checked={allSelected} onChange={() => toggleSeason(seasonVideos)} disabled={running} />
                  {season.title} ({seasonVideos.length})
                </label>
              </legend>
              {seasonVideos.map((video) => (
                <label key={video.id} className={processedVideoIds.has(video.id) ? "intro-video-processed" : ""}>
                  <input type="checkbox" checked={selected.has(video.id)} onChange={() => toggleVideo(video.id)} disabled={running} />
                  <span title={video.displayName}>{video.displayName}</span>
                  {processedVideoIds.has(video.id) && <small>analisado</small>}
                </label>
              ))}
            </fieldset>
          );
        })}
      </div>
      <div className="organize-row">
        <button title="Comparar os episódios selecionados" onClick={analyze} disabled={running || selected.size < 2}>
          Analisar selecionados ({selected.size})
        </button>
        {running && (
          <button title="Interromper a análise atual" className="danger" onClick={cancel}>
            Interromper análise
          </button>
        )}
      </div>
      {selected.size < 2 && <p className="intro-analysis-warning">Selecione pelo menos dois episódios para iniciar. Com menos de dois, nada será processado.</p>}
      {analysisMessage && (
        <p className="intro-analysis-result" aria-live="polite">
          {analysisMessage}
        </p>
      )}
      {progress && (
        <div className="thumbnail-progress" aria-live="polite">
          <div className="thumbnail-progress-head">
            <strong>
              {progress.processed}/{progress.total} episódios
            </strong>
            <span>{progress.cancelled ? "interrompido" : running ? "analisando" : "concluído"}</span>
          </div>
          {progress.total > 0 && <progress value={progress.processed} max={progress.total} />}
          <span className="thumbnail-progress-current">{progress.current}</span>
        </div>
      )}
      {suggestions.length > 0 && (
        <div className="intro-suggestions">
          <strong>Sugestões encontradas</strong>
          {suggestions.map((suggestion) => (
            <div key={suggestion.videoId} className="intro-suggestion">
              <span title={suggestion.displayName}>{suggestion.displayName}</span>
              <label>
                início
                <input type="number" min="0" step="0.1" value={suggestion.startSeconds} onChange={(event) => setSuggestions((current) => current.map((item) => (item.videoId === suggestion.videoId ? { ...item, startSeconds: Number(event.target.value) } : item)))} />
              </label>
              <label>
                fim
                <input type="number" min="0" step="0.1" value={suggestion.endSeconds} onChange={(event) => setSuggestions((current) => current.map((item) => (item.videoId === suggestion.videoId ? { ...item, endSeconds: Number(event.target.value) } : item)))} />
              </label>
              <span>
                {formatTime(suggestion.startSeconds)} - {formatTime(suggestion.endSeconds)} ({Math.round(suggestion.confidence * 100)}%)
              </span>
              <button title="Aceitar este intervalo para o episódio" onClick={() => accept(suggestion)}>
                Aceitar
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function CollectionPage({ collectionId, nav }: { collectionId: number; nav: Navigator }) {
  const [detail, setDetail] = useState<CollectionDetail | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [showOrganize, setShowOrganize] = useState(false);

  const refresh = useCallback(() => {
    api
      .getCollectionDetail(collectionId)
      .then(setDetail)
      .catch((e) => setStatus(String(e)));
  }, [collectionId]);

  useEffect(refresh, [refresh]);

  if (!detail) return <div className="page">{status ?? "Carregando…"}</div>;

  const play = (videoId: number) => nav.play(videoId, collectionId);
  // Marcadores de abertura são específicos de séries.
  const isMovie = detail.collectionType === "movie";
  const isSeries = detail.collectionType === "series";

  return (
    <div className="page">
      <header className="topbar">
        <button title="Voltar para a Biblioteca" onClick={() => nav.go({ name: "home" })}>
          ← Voltar
        </button>
        <h1>{detail.title}</h1>
        <button title="Mostrar ou ocultar as ferramentas deste catálogo" className={showOrganize ? "toggled" : ""} onClick={() => setShowOrganize((open) => !open)}>
          ⚙ Organizar
        </button>
      </header>
      {status && <p className="status">{status}</p>}

      {showOrganize && (
        <>
          <OrganizePanel collectionId={collectionId} title={detail.title} collectionType={detail.collectionType} showThumbnails={detail.showThumbnails} onChanged={refresh} onRemoved={() => nav.go({ name: "home" })} />
          {isSeries && <IntroAnalysisPanel collectionId={collectionId} seasons={detail.seasons} onStatus={setStatus} />}
        </>
      )}

      {detail.seasons.map(({ season, videos }) => (
        <section key={season.id}>
          <h2>
            {season.title}
            {isSeries && <IntroMarkerForm collectionId={collectionId} seasonId={season.id} onSaved={setStatus} />}
          </h2>
          <div className="episode-list">
            {videos.map((v) => (
              <VideoItem
                key={v.id}
                video={v}
                showThumbnails={detail.showThumbnails}
                onPlay={() => play(v.id)}
                onRenamed={(message) => {
                  setStatus(message);
                  refresh();
                }}
              />
            ))}
          </div>
        </section>
      ))}

      {detail.looseVideos.length > 0 && (
        <section>
          <h2>
            {isMovie ? "Arquivos" : detail.collectionType === "course" ? "Aulas avulsas" : "Vídeos avulsos"}
            {isSeries && <IntroMarkerForm collectionId={collectionId} seasonId={null} onSaved={setStatus} />}
          </h2>
          <div className="episode-list">
            {detail.looseVideos.map((v) => (
              <VideoItem
                key={v.id}
                video={v}
                showThumbnails={detail.showThumbnails}
                onPlay={() => play(v.id)}
                onRenamed={(message) => {
                  setStatus(message);
                  refresh();
                }}
              />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
