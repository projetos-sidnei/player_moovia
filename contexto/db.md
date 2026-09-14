# Banco de dados (spec §6)

PostgreSQL local — db `player_moovia`, user/senha `postgres`, localhost:5432. Acesso via sqlx (async, feature postgres). Credenciais só via `.env`.

## Schema (migrations)

```sql
CREATE TABLE library_roots (
    id SERIAL PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE collections (
    id SERIAL PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    folder_path TEXT NOT NULL UNIQUE,
    type TEXT NOT NULL DEFAULT 'series', -- 'series' | 'movie' (enum CollectionType no Rust)
    poster_path TEXT,                    -- futuro (TMDB)
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE seasons (
    id SERIAL PRIMARY KEY,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    folder_path TEXT NOT NULL UNIQUE,
    season_number INTEGER
);

CREATE TABLE videos (
    id SERIAL PRIMARY KEY,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    season_id INTEGER REFERENCES seasons(id) ON DELETE CASCADE, -- NULL = filme avulso
    file_path TEXT NOT NULL UNIQUE,      -- identidade primária do arquivo
    file_hash TEXT,                      -- opcional, recuperar progresso de arquivo movido (não-MVP)
    display_name TEXT NOT NULL,
    episode_number INTEGER,
    duration_seconds DOUBLE PRECISION,   -- preenchido na 1ª leitura via libVLC
    added_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE playback_progress (
    video_id INTEGER PRIMARY KEY REFERENCES videos(id) ON DELETE CASCADE,
    position_seconds DOUBLE PRECISION NOT NULL DEFAULT 0,
    watched BOOLEAN NOT NULL DEFAULT FALSE,
    last_played_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_videos_collection ON videos(collection_id);
CREATE INDEX idx_videos_season ON videos(season_id);

CREATE TABLE intro_markers (
    id SERIAL PRIMARY KEY,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    season_id INTEGER REFERENCES seasons(id) ON DELETE CASCADE, -- NULL = coleção inteira
    start_seconds DOUBLE PRECISION NOT NULL,
    end_seconds DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (collection_id, season_id)
);

CREATE TABLE user_settings (
    id SERIAL PRIMARY KEY,
    auto_skip_intro BOOLEAN NOT NULL DEFAULT FALSE
);
```

## Regras de negócio dos dados

- **Assistido:** `watched = true` automático quando `position_seconds >= 0.9 * duration_seconds`. Nunca antes, mesmo fechando o player.
- **Identidade do vídeo:** `file_path` (UPSERT por ele). `file_hash` é opcional/futuro.
- **Continue assistindo:** `watched = false AND position_seconds > 0`, ordem `last_played_at DESC`.
- **Prioridade intro marker:** match por `season_id` > match por `collection_id`.
