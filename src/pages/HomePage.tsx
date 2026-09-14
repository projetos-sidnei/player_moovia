import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import type { Navigator } from "../navigation";
import type { CollectionCard, ContinueWatchingItem } from "../types";
import { formatTime, progressRatio } from "../utils/time";

/** Capa gerada pelo usuário (carregada sob demanda, só quando existe). */
function CollectionPoster({ collection }: { collection: CollectionCard }) {
  const [poster, setPoster] = useState<string | null>(null);

  useEffect(() => {
    if (!collection.hasPoster) return;
    api
      .getCollectionPoster(collection.id)
      .then(setPoster)
      .catch(() => {});
  }, [collection.id, collection.hasPoster]);

  const unwatched = collection.totalVideos - collection.watchedVideos;
  return (
    <div className="card-poster">
      {poster && <img src={poster} alt="" className="card-poster-img" />}
      <span className="card-type">{collection.collectionType === "series" ? "Série" : collection.collectionType === "movie" ? "Filme" : "Curso"}</span>
      {unwatched > 0 && (
        <span className="badge">
          {unwatched} novo{unwatched > 1 ? "s" : ""}
        </span>
      )}
    </div>
  );
}

/** Ignora acentos e caixa para a busca ("rastreador" acha "O Rastreador"). */
function normalize(text: string): string {
  return text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

type LibraryFilter = "all" | "unwatched" | "in-progress" | "series" | "movie" | "course";

const FILTERS: { id: LibraryFilter; label: string }[] = [
  { id: "all", label: "Tudo" },
  { id: "unwatched", label: "Não assistidos" },
  { id: "in-progress", label: "Em andamento" },
  { id: "series", label: "Séries" },
  { id: "movie", label: "Filmes" },
  { id: "course", label: "Cursos" },
];

function matchesFilter(collection: CollectionCard, filter: LibraryFilter): boolean {
  switch (filter) {
    case "unwatched":
      return collection.watchedVideos < collection.totalVideos;
    case "in-progress":
      return collection.hasInProgress;
    case "series":
      return collection.collectionType === "series";
    case "movie":
      return collection.collectionType === "movie";
    case "course":
      return collection.collectionType === "course";
    default:
      return true;
  }
}

export function HomePage({ nav }: { nav: Navigator }) {
  const [collections, setCollections] = useState<CollectionCard[]>([]);
  const [continueWatching, setContinueWatching] = useState<ContinueWatchingItem[]>([]);
  const [scanning, setScanning] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<LibraryFilter>("all");
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedCollections, setSelectedCollections] = useState<Set<number>>(new Set());

  const refresh = useCallback(async () => {
    const [cols, cw] = await Promise.all([api.getCollections(), api.getContinueWatching()]);
    setCollections(cols);
    setContinueWatching(cw);
  }, []);

  useEffect(() => {
    refresh().catch((e) => setStatus(String(e)));
  }, [refresh]);

  async function addFolder() {
    const dir = await open({ directory: true, multiple: false, title: "Escolher pasta da biblioteca" });
    if (typeof dir !== "string") return;
    setScanning(true);
    setStatus("Escaneando…");
    try {
      const s = await api.scanLibrary(dir);
      setStatus(`Scan concluído: ${s.collections} coleções, ${s.videos} vídeos (${s.removed} removidos).`);
      await refresh();
    } catch (e) {
      setStatus(String(e));
    } finally {
      setScanning(false);
    }
  }

  async function rescan() {
    setScanning(true);
    setStatus("Re-escaneando…");
    try {
      const s = await api.rescanAll();
      setStatus(`Scan concluído: ${s.collections} coleções, ${s.videos} vídeos (${s.removed} removidos).`);
      await refresh();
    } catch (e) {
      setStatus(String(e));
    } finally {
      setScanning(false);
    }
  }

  function toggleCollection(collectionId: number) {
    setSelectedCollections((current) => {
      const next = new Set(current);
      if (next.has(collectionId)) next.delete(collectionId);
      else next.add(collectionId);
      return next;
    });
  }

  async function removeSelectedCollections() {
    if (selectedCollections.size === 0) return;
    const confirmed = window.confirm(`Remover ${selectedCollections.size} coleção(ões) da biblioteca? Os arquivos permanecerão no disco.`);
    if (!confirmed) return;
    setScanning(true);
    setStatus("Removendo coleções…");
    try {
      await Promise.all([...selectedCollections].map((id) => api.removeCollectionFromLibrary(id)));
      setSelectedCollections(new Set());
      setSelectionMode(false);
      setStatus("Coleções removidas da biblioteca. Os arquivos permanecem no disco.");
      await refresh();
    } catch (e) {
      setStatus(String(e));
    } finally {
      setScanning(false);
    }
  }

  const termo = normalize(search.trim());
  const visible = collections.filter((c) => matchesFilter(c, filter) && (termo === "" || normalize(c.title).includes(termo)));

  return (
    <div className="page home-page">
      <header className="topbar">
        <h1>Moovia</h1>
        <div className="topbar-actions">
          <button onClick={addFolder} disabled={scanning}>
            Adicionar pasta
          </button>
          <button onClick={rescan} disabled={scanning}>
            Re-escanear
          </button>
          <button onClick={() => nav.go({ name: "settings" })}>Configurações</button>
        </div>
      </header>

      <p className={`status home-status ${status ? "visible" : ""}`}>{status ?? " "}</p>

      {continueWatching.length > 0 && (
        <section className="home-continue">
          <h2>Continue assistindo</h2>
          <div className="cw-row">
            {continueWatching.map((item) => (
              <button key={item.videoId} className="cw-card" onClick={() => nav.play(item.videoId, item.collectionId)}>
                <span className="cw-collection">{item.collectionTitle}</span>
                <span className="cw-title">{item.displayName}</span>
                <span className="cw-time">
                  {formatTime(item.positionSeconds)}
                  {item.durationSeconds ? ` / ${formatTime(item.durationSeconds)}` : ""}
                </span>
                <span className="progress-track">
                  <span className="progress-fill" style={{ width: `${progressRatio(item.positionSeconds, item.durationSeconds) * 100}%` }} />
                </span>
              </button>
            ))}
          </div>
        </section>
      )}

      <section className="home-library">
        <h2 className="library-heading">
          Biblioteca
          <span className="library-tools">
            <input className="search" value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Buscar…" />
            {FILTERS.map((f) => (
              <button key={f.id} className={`chip ${filter === f.id ? "toggled" : ""}`} onClick={() => setFilter(f.id)}>
                {f.label}
              </button>
            ))}
            <button
              className={selectionMode ? "toggled" : ""}
              onClick={() => {
                setSelectionMode((enabled) => !enabled);
                setSelectedCollections(new Set());
              }}
              disabled={scanning}
            >
              {selectionMode ? "Cancelar seleção" : "Selecionar"}
            </button>
            {selectionMode && (
              <button onClick={removeSelectedCollections} disabled={scanning || selectedCollections.size === 0}>
                Remover selecionadas ({selectedCollections.size})
              </button>
            )}
          </span>
        </h2>
        {collections.length === 0 && !scanning && <p className="empty">Nenhuma coleção. Clique em “Adicionar pasta” para escanear sua biblioteca.</p>}
        {collections.length > 0 && visible.length === 0 && <p className="empty">Nada encontrado com esse filtro.</p>}
        <div className="grid library-grid">
          {visible.map((col) => {
            const selected = selectedCollections.has(col.id);
            return (
              <button key={col.id} className={`card ${selected ? "selected" : ""}`} aria-pressed={selectionMode ? selected : undefined} onClick={() => (selectionMode ? toggleCollection(col.id) : nav.go({ name: "collection", collectionId: col.id }))}>
                <CollectionPoster collection={col} />
                <div className="card-title" title={col.title}>
                  {col.title}
                </div>
                <div className="card-sub">
                  {col.watchedVideos}/{col.totalVideos} assistidos
                </div>
                <span className="progress-track">
                  <span className="progress-fill" style={{ width: `${col.totalVideos > 0 ? (col.watchedVideos / col.totalVideos) * 100 : 0}%` }} />
                </span>
                {selectionMode && <span className="card-selection">{selected ? "Selecionado" : "Selecionar"}</span>}
              </button>
            );
          })}
        </div>
      </section>
    </div>
  );
}
