import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import { EVENTS } from "./constants/events.constants";
import type { Screen } from "./navigation";
import { SettingsProvider } from "./store/settings";
import { HomePage } from "./pages/HomePage";
import { CollectionPage } from "./pages/CollectionPage";
import { SettingsPage } from "./pages/SettingsPage";
import "./App.css";

function App() {
  const [screen, setScreen] = useState<Screen>({ name: "home" });
  // Fechar o player altera progresso/assistido: remonta a tela para recarregar.
  const [refreshKey, setRefreshKey] = useState(0);
  // Enquanto o player está aberto esta janela fica preta: ela é o que aparece
  // nas barras pretas de filmes em cinemascope, atrás da janela do vídeo.
  const [playing, setPlaying] = useState(false);

  const play = useCallback((videoId: number, collectionId: number) => {
    setScreen({ name: "collection", collectionId });
    setPlaying(true);
    api.openPlayer(videoId, collectionId).catch((e) => {
      setPlaying(false);
      console.error(e);
    });
  }, []);

  useEffect(() => {
    const unlisten = listen(EVENTS.PLAYER_CLOSED, () => {
      setPlaying(false);
      setRefreshKey((k) => k + 1);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const nav = { go: setScreen, play };

  if (playing) return <div className="playing-backdrop" />;

  return (
    <SettingsProvider>
      {screen.name === "home" && <HomePage key={refreshKey} nav={nav} />}
      {screen.name === "collection" && (
        <CollectionPage key={`${screen.collectionId}-${refreshKey}`} collectionId={screen.collectionId} nav={nav} />
      )}
      {screen.name === "settings" && <SettingsPage nav={nav} />}
    </SettingsProvider>
  );
}

export default App;
