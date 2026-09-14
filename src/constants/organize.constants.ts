// Renomeação em lote e capa — espelham os valores de constants.rs.

export const RENAME_TOKENS = {
  SERIES: "{serie}",
  SEASON: "{temporada}",
  EPISODE: "{episodio}",
  /** Nome do episódio já separado das tags de release. */
  EPISODE_NAME: "{nome}",
  /** Nome bruto do arquivo, como está hoje. */
  TITLE: "{titulo}",
} as const;

export const DEFAULT_RENAME_PATTERN = "S{temporada}E{episodio} - {nome}";

/** Instante padrão sugerido para a capa (evita logo/abertura). */
export const POSTER_DEFAULT_SECONDS = 300;

/** Avanço usado para procurar outro frame ao reprocessar a capa. */
export const POSTER_REPROCESS_STEP_SECONDS = 30;
