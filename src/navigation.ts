// "Rotas" da aplicação (sem router) — telas do MVP, spec §10.
// O player NÃO é uma tela daqui: ele roda na janela sobreposta (ver PlayerPage).

export type Screen =
  | { name: "home" }
  | { name: "collection"; collectionId: number }
  | { name: "settings" };

export interface Navigator {
  go: (screen: Screen) => void;
  /** Abre o player na janela sobreposta, sobre o vídeo nativo. */
  play: (videoId: number, collectionId: number) => void;
}
