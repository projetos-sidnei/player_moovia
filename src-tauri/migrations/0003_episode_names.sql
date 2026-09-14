-- Nome de exibição definido pelo usuário (extraído do arquivo ou digitado à mão)
-- não pode ser sobrescrito pelo re-scan nem pela renomeação em lote.
ALTER TABLE videos ADD COLUMN display_name_locked BOOLEAN NOT NULL DEFAULT FALSE;
