import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { OVERLAY_WINDOW_LABEL } from "./constants/player.constants";
import { PlayerPage } from "./pages/PlayerPage";
import { SettingsProvider } from "./store/settings";
import "./App.css";

// A janela sobreposta do player carrega o mesmo bundle: ela renderiza só os
// controles, com fundo transparente, sobre a janela nativa do vídeo.
// O label é a fonte da verdade (a query string traz apenas o alvo).
function isOverlayWindow(): boolean {
  try {
    return getCurrentWindow().label === OVERLAY_WINDOW_LABEL;
  } catch {
    return false;
  }
}

const params = new URLSearchParams(window.location.search);

// Sem StrictMode: o double-mount de dev dispararia play/stop duplicado no libVLC.
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

if (isOverlayWindow()) {
  document.body.classList.add("player-mode");
  root.render(
    <SettingsProvider>
      <PlayerPage
        videoId={Number(params.get("videoId"))}
        collectionId={Number(params.get("collectionId"))}
      />
    </SettingsProvider>,
  );
} else {
  root.render(<App />);
}
