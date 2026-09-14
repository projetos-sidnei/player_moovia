import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { Navigator } from "../navigation";
import type { LibraryRoot } from "../types";
import { useSettingsStore } from "../store/settings";

/** Pasta da biblioteca com remoção confirmada em dois passos. */
function RootRow({ root, onRemoved }: { root: LibraryRoot; onRemoved: (message: string) => void }) {
  const [confirming, setConfirming] = useState(false);

  async function remove() {
    try {
      await api.removeLibraryRoot(root.id);
      onRemoved("Pasta removida da biblioteca (os arquivos continuam no disco).");
    } catch (e) {
      onRemoved(String(e));
    }
  }

  return (
    <li className="root-row">
      <span className="root-path">{root.path}</span>
      {confirming ? (
        <span className="root-actions">
          <button className="danger" onClick={remove}>
            Remover
          </button>
          <button onClick={() => setConfirming(false)}>Cancelar</button>
        </span>
      ) : (
        <button
          className="episode-edit"
          title="Remover pasta da biblioteca"
          onClick={() => setConfirming(true)}
        >
          🗑
        </button>
      )}
    </li>
  );
}

export function SettingsPage({ nav }: { nav: Navigator }) {
  const { autoSkipIntro, setAutoSkipIntro } = useSettingsStore();
  const [roots, setRoots] = useState<LibraryRoot[]>([]);
  const [status, setStatus] = useState<string | null>(null);

  const refresh = useCallback(() => {
    api.getLibraryRoots().then(setRoots).catch((e) => setStatus(String(e)));
  }, []);

  useEffect(refresh, [refresh]);

  return (
    <div className="page">
      <header className="topbar">
        <button onClick={() => nav.go({ name: "home" })}>← Voltar</button>
        <h1>Configurações</h1>
      </header>
      {status && <p className="status">{status}</p>}

      <section>
        <label className="setting-row">
          <input
            type="checkbox"
            checked={autoSkipIntro}
            onChange={(e) => setAutoSkipIntro(e.target.checked).catch((err) => setStatus(String(err)))}
          />
          Pular abertura automaticamente
        </label>
      </section>

      <section>
        <h2>Pastas da biblioteca</h2>
        <p className="hint">
          Remover uma pasta apaga só o que está catalogado dela — nenhum arquivo é excluído do disco.
        </p>
        {roots.length === 0 ? (
          <p className="empty">Nenhuma pasta cadastrada.</p>
        ) : (
          <ul className="root-list">
            {roots.map((r) => (
              <RootRow
                key={r.id}
                root={r}
                onRemoved={(message) => {
                  setStatus(message);
                  refresh();
                }}
              />
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
