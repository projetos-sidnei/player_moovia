-- Schema inicial do player_moovia (spec §6)

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
    type TEXT NOT NULL DEFAULT 'series', -- 'series' | 'movie'
    poster_path TEXT,
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
    season_id INTEGER REFERENCES seasons(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL UNIQUE,
    file_hash TEXT,
    display_name TEXT NOT NULL,
    episode_number INTEGER,
    duration_seconds DOUBLE PRECISION,
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
    season_id INTEGER REFERENCES seasons(id) ON DELETE CASCADE,
    start_seconds DOUBLE PRECISION NOT NULL,
    end_seconds DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE NULLS NOT DISTINCT (collection_id, season_id)
);

CREATE TABLE user_settings (
    id SERIAL PRIMARY KEY,
    auto_skip_intro BOOLEAN NOT NULL DEFAULT FALSE
);

-- Tabela de configuração global de linha única: já nasce com a linha default.
INSERT INTO user_settings (auto_skip_intro) VALUES (FALSE);
