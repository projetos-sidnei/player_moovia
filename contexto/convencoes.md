# Convenções de código (spec §3–§5)

## Regra de ouro

Nenhum literal (número, string, path, nome de evento/comando/tabela) solto na lógica. Antes de digitar um literal: "já existe em constants?" → se não, criar lá e importar. Nunca duplicar.

## Backend (Rust)

- `src-tauri/src/constants.rs` centraliza:
  - `const VIDEO_EXTENSIONS: &[&str]` = mkv, mp4, avi, mov, wmv, m4v, webm
  - `const WATCHED_THRESHOLD_RATIO: f64 = 0.9`
  - `const PROGRESS_SAVE_INTERVAL_SECS: u64 = 5`
  - Nomes de eventos Tauri (emit/listen) como consts de string.
- Config: `.env` (dotenvy) lido UMA vez no `main.rs` → `struct AppConfig` → injetado via Tauri `State`. Nunca `std::env::var` espalhado.
- Enums Rust para valores fechados (ex. `enum CollectionType { Series, Movie }`); conversão de/para TEXT do Postgres só na borda (`db/models.rs`).
- SQL só dentro de arquivos de `db/`.

## Frontend (TS)

- `src/constants/` por domínio: `player.constants.ts`, `routes.constants.ts`, `ipc.constants.ts` (nomes de comandos `invoke` SEMPRE importados de constante única).
- Interfaces TS espelhando structs Rust para retornos de `invoke` — zero `any`.
- Settings do usuário (ex. `auto_skip_intro`) num único store/hook (`useSettingsStore`).

## Estrutura de pastas alvo

```
src-tauri/src/
  main.rs, constants.rs
  commands/{scanner,player,library}.rs
  db/{mod,models}.rs + migrations/
  vlc/engine.rs
  scan/{walker,natural_sort}.rs
src/  (frontend)
  components/ pages/ constants/ main.tsx
```

## .env

```
DATABASE_URL=postgres://postgres:postgres@localhost:5432/player_moovia
```
`.env` no `.gitignore`; `.env.example` versionado.
