// Organização manual da coleção: nome, renomeação em lote dos
// arquivos e capa gerada de um frame. Nada roda sozinho — tudo por botão.

import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { DEFAULT_RENAME_PATTERN, POSTER_DEFAULT_SECONDS, POSTER_REPROCESS_STEP_SECONDS, RENAME_TOKENS } from "../constants/organize.constants";
import { EVENTS } from "../constants/events.constants";
import type { CollectionCard, CollectionType, EpisodeNamePlanItem, RenamePlanItem, ThumbnailProgress } from "../types";
import { formatTime } from "../utils/time";

export function OrganizePanel({
  collectionId,
  title,
  collectionType,
  showThumbnails,
  onChanged,
  onRemoved,
}: {
  collectionId: number;
  title: string;
  collectionType: CollectionType;
  showThumbnails: boolean;
  onChanged: () => void;
  /** A coleção deixou de existir — a tela precisa sair daqui. */
  onRemoved: () => void;
}) {
  const [newTitle, setNewTitle] = useState(title);
  const [pattern, setPattern] = useState(DEFAULT_RENAME_PATTERN);
  const [plan, setPlan] = useState<RenamePlanItem[] | null>(null);
  const [namePlan, setNamePlan] = useState<EpisodeNamePlanItem[] | null>(null);
  const [posterSeconds, setPosterSeconds] = useState(String(POSTER_DEFAULT_SECONDS));
  const [poster, setPoster] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [confirmingCollection, setConfirmingCollection] = useState(false);
  const [others, setOthers] = useState<CollectionCard[]>([]);
  const [mergeTarget, setMergeTarget] = useState("");
  const [thumbnailProgress, setThumbnailProgress] = useState<ThumbnailProgress | null>(null);
  const [thumbnailStartedAt, setThumbnailStartedAt] = useState<number | null>(null);
  const [thumbnailCancelRequested, setThumbnailCancelRequested] = useState(false);
  const thumbnailCancelRef = useRef(false);

  useEffect(() => {
    api
      .getCollections()
      .then((all) => setOthers(all.filter((c) => c.id !== collectionId)))
      .catch(() => {});
  }, [collectionId]);

  useEffect(() => {
    const unlisten = listen<ThumbnailProgress>(EVENTS.THUMBNAIL_PROGRESS, (event) => {
      if (event.payload.collectionId === collectionId) setThumbnailProgress(event.payload);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [collectionId]);

  async function changeType(next: CollectionType) {
    const done = await run("Salvando…", () => api.setCollectionType(collectionId, next));
    if (done !== undefined) {
      setStatus(next === "series" ? "Catalogado como série." : next === "movie" ? "Catalogado como filme." : "Catalogado como curso.");
      onChanged();
    }
  }

  async function mergeInto() {
    const targetId = Number(mergeTarget);
    if (!targetId) {
      setStatus("Escolha a coleção de destino.");
      return;
    }
    const done = await run("Agrupando…", () => api.mergeCollectionInto(collectionId, targetId));
    if (done !== undefined) onRemoved();
  }

  async function run<T>(label: string, action: () => Promise<T>): Promise<T | undefined> {
    setBusy(true);
    setStatus(label);
    try {
      const result = await action();
      return result;
    } catch (e) {
      setStatus(String(e));
      return undefined;
    } finally {
      setBusy(false);
    }
  }

  async function saveTitle() {
    const done = await run("Salvando nome…", () => api.setCollectionTitle(collectionId, newTitle));
    if (done !== undefined) {
      setStatus("Nome salvo. Ele não será sobrescrito pelo próximo scan.");
      onChanged();
    }
  }

  async function previewNames() {
    const result = await run("Lendo os arquivos…", () => api.previewEpisodeNames(collectionId));
    if (result) {
      setNamePlan(result);
      setStatus(null);
    }
  }

  async function applyNames() {
    const applied = await run("Salvando nomes…", () => api.applyEpisodeNames(collectionId));
    if (applied !== undefined) {
      setStatus(`${applied} nome(s) atualizados.`);
      setNamePlan(null);
      onChanged();
    }
  }

  async function previewRename() {
    const result = await run("Calculando…", () => api.previewRename(collectionId, pattern));
    if (result) {
      setPlan(result);
      setStatus(null);
    }
  }

  async function applyRename() {
    const outcome = await run("Renomeando…", () => api.applyRename(collectionId, pattern));
    if (outcome) {
      const errors = outcome.errors.length > 0 ? ` Erros: ${outcome.errors.join("; ")}` : "";
      setStatus(`${outcome.renamed} renomeado(s), ${outcome.skipped} ignorado(s).${errors}`);
      setPlan(null);
      onChanged();
    }
  }

  async function generatePosterAt(seconds: number) {
    setPosterSeconds(String(seconds));
    const uri = await run("Capturando frame…", () => api.generateCollectionPoster(collectionId, seconds));
    if (uri) {
      setPoster(uri);
      setStatus("Capa gerada.");
      onChanged();
    }
  }

  async function generatePoster() {
    const seconds = Number(posterSeconds);
    if (Number.isNaN(seconds) || seconds < 0) {
      setStatus("Informe o instante em segundos.");
      return;
    }
    await generatePosterAt(seconds);
  }

  async function reprocessPoster() {
    const currentSeconds = Number(posterSeconds);
    const seconds = Number.isFinite(currentSeconds) && currentSeconds >= 0 ? currentSeconds + POSTER_REPROCESS_STEP_SECONDS : POSTER_DEFAULT_SECONDS;
    await generatePosterAt(seconds);
  }

  async function setThumbnailsVisible(visible: boolean) {
    const done = await run(visible ? "Mostrando miniaturas…" : "Ocultando miniaturas…", () => api.setCollectionThumbnailsVisible(collectionId, visible));
    if (done !== undefined) onChanged();
  }

  async function generateThumbnails(force: boolean) {
    thumbnailCancelRef.current = false;
    setThumbnailCancelRequested(false);
    setThumbnailStartedAt(Date.now());
    setThumbnailProgress({ collectionId, processed: 0, total: 0, generated: 0, failed: 0, current: "Preparando..." });
    const geradas = await run(force ? "Reprocessando miniaturas…" : "Processando miniaturas faltantes…", () => api.generateEpisodeThumbnails(collectionId, force));
    if (geradas !== undefined) {
      setStatus(thumbnailCancelRef.current ? `Processamento interrompido. ${geradas} miniatura(s) processadas.` : `${geradas} miniatura(s) processadas.`);
      onChanged();
    }
  }

  async function cancelThumbnails() {
    thumbnailCancelRef.current = true;
    setThumbnailCancelRequested(true);
    setStatus("Interrompendo após a captura atual…");
    await api.cancelEpisodeThumbnails().catch((e) => setStatus(String(e)));
  }

  async function removeCollection() {
    const done = await run("Removendo…", () => api.removeCollectionFromLibrary(collectionId));
    if (done !== undefined) {
      setConfirmingCollection(false);
      onRemoved();
    }
  }

  async function deleteCollection() {
    const outcome = await run("Enviando para a Lixeira…", () => api.deleteCollectionFiles(collectionId));
    if (!outcome) return;
    if (outcome.errors.length > 0) {
      setStatus(`Nada foi removido da biblioteca. Erros: ${outcome.errors.join("; ")}`);
      return;
    }
    setConfirmingCollection(false);
    onRemoved();
  }

  const changes = plan?.filter((p) => !p.unchanged) ?? [];
  const conflicts = changes.filter((p) => p.conflict).length;
  const nameChanges = namePlan?.filter((p) => !p.unchanged && !p.locked) ?? [];
  const elapsedSeconds = thumbnailStartedAt ? Math.max(0, (Date.now() - thumbnailStartedAt) / 1000) : 0;
  const averageSeconds = thumbnailProgress && thumbnailProgress.processed > 0 ? elapsedSeconds / thumbnailProgress.processed : 0;
  const remainingSeconds = thumbnailProgress ? Math.max(0, (thumbnailProgress.total - thumbnailProgress.processed) * averageSeconds) : 0;
  const formatElapsed = (seconds: number) => {
    const rounded = Math.ceil(seconds);
    const minutes = Math.floor(rounded / 60);
    const rest = rounded % 60;
    return minutes > 0 ? `${minutes}m ${rest}s` : `${rest}s`;
  };

  return (
    <section className="organize">
      <div className="organize-block">
        <h3>Nome da coleção</h3>
        <p className="hint">Use quando o nome da pasta não identifica a série ou o anime.</p>
        <div className="organize-row">
          <input className="grow" value={newTitle} onChange={(e) => setNewTitle(e.target.value)} placeholder="Ex.: Bleach: Thousand-Year Blood War" />
          <button title="Salvar o nome personalizado do catálogo" onClick={saveTitle} disabled={busy || newTitle.trim() === ""}>
            Salvar nome
          </button>
        </div>
      </div>

      <div className="organize-block">
        <h3>Catalogação</h3>
        <p className="hint">Corrija o que o scan classificou errado, ou transforme esta coleção numa temporada de outra (útil quando a mesma série entrou em pedaços separados).</p>
        <div className="organize-row">
          <span className="hint" style={{ margin: 0 }}>
            Tipo:
          </span>
          <button title="Classificar este catálogo como série" className={collectionType === "series" ? "toggled" : ""} onClick={() => changeType("series")} disabled={busy}>
            Série
          </button>
          <button title="Classificar este catálogo como filme" className={collectionType === "movie" ? "toggled" : ""} onClick={() => changeType("movie")} disabled={busy}>
            Filme
          </button>
          <button title="Classificar este catálogo como curso" className={collectionType === "course" ? "toggled" : ""} onClick={() => changeType("course")} disabled={busy}>
            Curso
          </button>
        </div>
        <div className="organize-row">
          <span className="hint" style={{ margin: 0 }}>
            Agrupar como temporada de:
          </span>
          <select value={mergeTarget} onChange={(e) => setMergeTarget(e.target.value)}>
            <option value="">Escolher coleção…</option>
            {others.map((c) => (
              <option key={c.id} value={c.id}>
                {c.title}
              </option>
            ))}
          </select>
          <button title="Mover o conteúdo para a coleção escolhida" onClick={mergeInto} disabled={busy || mergeTarget === ""}>
            Agrupar
          </button>
        </div>
      </div>

      <div className="organize-block">
        <h3>Nomes dos episódios</h3>
        <p className="hint">
          Separa o nome do episódio das tags de release do arquivo (<code>…S01E01.THE.BLOOD.WARFARE.1080p.WEB-DL</code> → <code>The Blood Warfare</code>). Nomes que você editou à mão não são tocados.
        </p>
        <div className="organize-row">
          <button title="Pré-visualizar os nomes sugeridos para os episódios" onClick={previewNames} disabled={busy}>
            Pré-visualizar nomes
          </button>
          <button title="Aplicar os nomes sugeridos aos episódios" onClick={applyNames} disabled={busy || nameChanges.length === 0}>
            Aplicar ({nameChanges.length})
          </button>
        </div>

        {namePlan && (
          <div className="rename-plan">
            {namePlan.length === 0 ? (
              <p className="hint">Nenhum arquivo desta coleção tem nome de episódio identificável — use a edição manual na lista de episódios.</p>
            ) : nameChanges.length === 0 ? (
              <p className="hint">Todos os nomes já estão como o parser sugere.</p>
            ) : (
              <ul>
                {nameChanges.map((item) => (
                  <li key={item.videoId}>
                    <span className="from">{item.currentName}</span>
                    <span className="arrow">→</span>
                    <span className="to">{item.newName}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>

      <div className="organize-block">
        <h3>Renomear arquivos</h3>
        <p className="hint">
          Marcadores: <code>{RENAME_TOKENS.SERIES}</code> <code>{RENAME_TOKENS.SEASON}</code> <code>{RENAME_TOKENS.EPISODE}</code> <code>{RENAME_TOKENS.EPISODE_NAME}</code> (nome do episódio) <code>{RENAME_TOKENS.TITLE}</code> (nome bruto do arquivo) — o padrão vale para todos os episódios da coleção. Nada é alterado até você aplicar.
        </p>
        <div className="organize-row">
          <input className="grow" value={pattern} onChange={(e) => setPattern(e.target.value)} />
          <button title="Pré-visualizar a renomeação dos arquivos" onClick={previewRename} disabled={busy}>
            Pré-visualizar
          </button>
          <button title="Aplicar a renomeação aos arquivos" onClick={applyRename} disabled={busy || changes.length === 0}>
            Aplicar ({changes.length - conflicts})
          </button>
        </div>

        {plan && (
          <div className="rename-plan">
            {changes.length === 0 ? (
              <p className="hint">Nenhum arquivo mudaria de nome com esse padrão.</p>
            ) : (
              <>
                {conflicts > 0 && <p className="hint warn">{conflicts} arquivo(s) serão ignorados: já existe outro com o nome de destino.</p>}
                <ul>
                  {changes.map((item) => (
                    <li key={item.videoId} className={item.conflict ? "conflict" : ""}>
                      <span className="from">{item.currentName}</span>
                      <span className="arrow">→</span>
                      <span className="to">{item.newName}</span>
                      {item.conflict && <span className="tag">já existe</span>}
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
        )}
      </div>

      <div className="organize-block">
        <h3>Capa</h3>
        <p className="hint">Captura um frame do primeiro episódio no instante indicado ({formatTime(Number(posterSeconds) || 0)}) e usa como capa na biblioteca.</p>
        <div className="organize-row">
          <input value={posterSeconds} onChange={(e) => setPosterSeconds(e.target.value)} placeholder="segundos" />
          <button title="Capturar e salvar uma capa no instante informado" onClick={generatePoster} disabled={busy}>
            Gerar capa
          </button>
          <button title="Capturar outro frame para substituir a capa" onClick={reprocessPoster} disabled={busy}>
            Reprocessar outro frame
          </button>
          {poster && <img className="poster-preview" src={poster} alt="Capa gerada" />}
        </div>
      </div>

      <div className="organize-block">
        <h3>Miniaturas dos episódios</h3>
        <p className="hint">As miniaturas ficam salvas em cache. Você pode ocultá-las neste catálogo, processar apenas as que faltam ou reprocessar todas.</p>
        <div className="organize-row">
          <button title={showThumbnails ? "Ocultar miniaturas deste catálogo" : "Mostrar miniaturas deste catálogo"} onClick={() => setThumbnailsVisible(!showThumbnails)} disabled={busy}>
            {showThumbnails ? "Ocultar miniaturas" : "Mostrar miniaturas"}
          </button>
          <button title="Gerar somente as miniaturas que ainda não existem" onClick={() => generateThumbnails(false)} disabled={busy}>
            Processar faltantes
          </button>
          <button title="Regenerar todas as miniaturas deste catálogo" onClick={() => generateThumbnails(true)} disabled={busy}>
            Reprocessar todas
          </button>
          {busy && thumbnailProgress && (
            <button title="Interromper o processamento depois da captura atual" className="danger" onClick={cancelThumbnails} disabled={thumbnailCancelRequested}>
              {thumbnailCancelRequested ? "Interrompendo…" : "Interromper"}
            </button>
          )}
        </div>
        {thumbnailProgress && (
          <div className="thumbnail-progress" aria-live="polite">
            <div className="thumbnail-progress-head">
              <strong>{thumbnailProgress.total > 0 ? `${thumbnailProgress.processed}/${thumbnailProgress.total} vídeos` : "Preparando processamento…"}</strong>
              <span>
                {thumbnailProgress.generated} geradas
                {thumbnailProgress.failed > 0 ? ` · ${thumbnailProgress.failed} falhas` : ""}
                {thumbnailProgress.cancelled ? " · interrompido" : ""}
              </span>
            </div>
            {thumbnailProgress.total > 0 && <progress value={thumbnailProgress.processed} max={thumbnailProgress.total} />}
            <div className="thumbnail-progress-times">
              <span>Decorrido: {formatElapsed(elapsedSeconds)}</span>
              <span>Estimativa restante: {thumbnailProgress.processed > 0 ? formatElapsed(remainingSeconds) : "calculando…"}</span>
            </div>
            <span className="thumbnail-progress-current">{thumbnailProgress.current}</span>
          </div>
        )}
      </div>

      <div className="organize-block danger-zone">
        <h3>Excluir coleção</h3>
        <p className="hint">“Só do player” apaga apenas os registros — os arquivos ficam no disco e voltam num próximo scan. “Excluir do PC” manda todos os vídeos para a Lixeira do Windows (dá para recuperar por lá) e remove as pastas que ficarem vazias.</p>
        {confirmingCollection ? (
          <div className="organize-row">
            <span className="confirm-text">
              Excluir <strong>{title}</strong>?
            </span>
            <button title="Remover o catálogo somente do player, preservando os arquivos" onClick={removeCollection} disabled={busy}>
              Só do player
            </button>
            <button title="Enviar os vídeos do catálogo para a Lixeira" className="danger" onClick={deleteCollection} disabled={busy}>
              Excluir do PC
            </button>
            <button title="Cancelar a exclusão do catálogo" onClick={() => setConfirmingCollection(false)} disabled={busy}>
              Cancelar
            </button>
          </div>
        ) : (
          <button title="Abrir as opções de exclusão do catálogo" className="danger" onClick={() => setConfirmingCollection(true)} disabled={busy}>
            🗑 Excluir coleção…
          </button>
        )}
      </div>

      {status && <p className="status">{status}</p>}
    </section>
  );
}
