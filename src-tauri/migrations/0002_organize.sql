-- Título definido à mão (série/anime cujo nome não vem da pasta) não pode ser
-- sobrescrito pelo re-scan.
ALTER TABLE collections ADD COLUMN title_locked BOOLEAN NOT NULL DEFAULT FALSE;
