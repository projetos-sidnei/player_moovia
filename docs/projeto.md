# player_moovia — Especificação Técnica para Agente de Desenvolvimento

## 1. Visão Geral

Aplicação desktop para Windows que funciona como um player de biblioteca pessoal de vídeos, no estilo "continue assistindo" da Netflix. O usuário aponta um diretório raiz no disco, a aplicação escaneia recursivamente essa pasta, monta uma biblioteca (séries/temporadas/episódios ou filmes avulsos) a partir da própria estrutura de pastas, reproduz os vídeos com suporte total a codecs (incluindo MKV/HEVC), e persiste o progresso de reprodução e o status de "assistido" por arquivo.

Não é um app web. É um app desktop nativo com UI em tecnologia web embutida (WebView do sistema), sem depender de navegador nem de sandbox de File System Access.

## 2. Stack Definida

| Camada                         | Tecnologia                                                                                                                                         |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shell / runtime desktop        | **Tauri 2.x** (Rust)                                                                                                                               |
| Backend / lógica de negócio    | **Rust**                                                                                                                                           |
| Engine de reprodução de vídeo  | **libVLC** via bindings `vlc-rs` (usa a instalação de VLC já presente na máquina)                                                                  |
| Banco de dados                 | **PostgreSQL** já instalado localmente — database `player_moovia`, usuário `postgres`                                                              |
| Driver de banco (Rust)         | `sqlx` (async, com feature `postgres`, macros compile-time opcionalmente)                                                                          |
| Frontend (UI)                  | HTML/CSS/JS (Vite). Framework sugerido: **React** ou **Svelte** — livre para o agente escolher o que já tiver mais suporte no template Tauri atual |
| Comunicação frontend ↔ backend | `tauri::command` (invoke) + eventos (`emit`/`listen`) para progresso de reprodução em tempo real                                                   |

### Por que essa combinação

- Tauri usa o WebView2 do Windows: binário leve, baixo consumo de RAM, não é Electron.
- A tag `<video>` do WebView2 **não** reproduz MKV/HEVC de forma confiável — por isso o vídeo **não** é tocado via `<video>` do HTML. A reprodução é feita pelo **libVLC renderizando na janela nativa** (via handle HWND), e a UI web controla o VLC por comandos (play, pause, seek, get_position) através de `tauri::command`.
- PostgreSQL já está disponível, então não há necessidade de SQLite — usar o banco existente reduz uma dependência.

## 3. Credenciais e Configuração

- Database: `player_moovia`
- Usuário: `postgres`
- Senha: `postgres`
- Host: `localhost` (assumir porta padrão `5432`, salvo indicação em contrário)

**Regra para o agente:** nunca hardcodar essas credenciais direto no código-fonte. Usar variável de ambiente via arquivo `.env` na raiz do projeto (não versionado — adicionar ao `.gitignore`), lido no `main.rs` com o crate `dotenvy`. Criar um `.env.example` versionado com os nomes das variáveis (sem os valores reais preenchidos, ou com os valores de dev já que é ambiente local — decisão do time, mas o `.env` real nunca vai pro git).

```
# .env.example
DATABASE_URL=postgres://postgres:postgres@localhost:5432/player_moovia
```

## 4. Convenções de Código — Sem Hardcode, Tudo Tipado e Centralizado

Regra geral para o agente seguir em **todo** o código gerado, backend e frontend: nenhum valor literal solto pelo meio da lógica (magic numbers, magic strings, paths, extensões de arquivo, nomes de tabela, chaves de configuração, etc.). Tudo deve vir de um ponto único de definição, e esse ponto deve ser tipado — não um `any`/string solta.

### 4.1 Backend (Rust)

- Criar `src-tauri/src/constants.rs` (ou um módulo `config/`) centralizando:
  - Extensões de vídeo aceitas (`const VIDEO_EXTENSIONS: &[&str]`).
  - Threshold de "assistido" (`const WATCHED_THRESHOLD_RATIO: f64 = 0.9`).
  - Intervalo de salvamento de progresso (`const PROGRESS_SAVE_INTERVAL_SECS: u64 = 5`).
  - Nomes de eventos emitidos via Tauri (`emit`/`listen`) como constantes de string, não repetidas em cada `.rs` que dispara ou escuta o evento.
- Nunca escrever string de conexão, credenciais ou paths de sistema direto no código — sempre via `.env` + `struct AppConfig` carregada uma vez na inicialização (`main.rs`) e passada por injeção (Tauri `State`), não relida do ambiente em cada função.
- Enums do Rust (não strings soltas) para valores fechados que hoje estão como `TEXT` no banco, ex.: tipo de coleção (`enum CollectionType { Series, Movie }`) — converter de/para o `TEXT` do Postgres na borda (camada `db/models.rs`), mas usar o enum em toda a lógica de negócio.
- IDs de tabela, nomes de coluna usados em queries dinâmicas (se houver) também centralizados — evitar strings de SQL montadas manualmente fora dos arquivos de `db/`.

### 4.2 Frontend (TS/JS)

- Criar `src/constants/` com arquivos separados por domínio, por exemplo:
  - `player.constants.ts` (intervalos de polling, throttle, etc.)
  - `routes.constants.ts` (paths de rota, se houver client-side routing)
  - `ipc.constants.ts` (nomes dos comandos Tauri invocados via `invoke("nome_do_comando")` — o nome do comando **nunca** deve ser digitado como string solta em múltiplos componentes, sempre importado de uma constante única).
- Usar TypeScript com tipos/interfaces espelhando as structs do Rust para os retornos de `invoke` (ex.: `interface Video { id: number; filePath: string; ... }`), evitando `any` e evitando desestruturar campos de uma resposta sem tipo declarado.
- Configurações do usuário (como `auto_skip_intro`) devem ter um único hook/store centralizado (ex. `useSettingsStore`), não lidas/escritas em múltiplos componentes de forma independente.

### 4.3 Regra prática para o agente

Antes de introduzir qualquer literal (número, string, path, nome de evento/comando) em mais de um lugar do código, o agente deve perguntar: _"esse valor já existe em `constants.rs` / `constants/`?"_ — se não existir, criar lá primeiro e importar, nunca duplicar o literal.

## 5. Estrutura de Pastas do Projeto

```
player_moovia/
├── src-tauri/              # Backend Rust
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands/       # tauri::command exposed to frontend
│   │   │   ├── scanner.rs
│   │   │   ├── player.rs
│   │   │   └── library.rs
│   │   ├── db/
│   │   │   ├── mod.rs
│   │   │   ├── models.rs
│   │   │   └── migrations/
│   │   ├── vlc/
│   │   │   └── engine.rs
│   │   └── scan/
│   │       ├── walker.rs   # scan recursivo
│   │       └── natural_sort.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                    # Frontend
│   ├── components/
│   ├── pages/
│   └── main.tsx (ou main.ts)
├── .env.example
├── .gitignore
└── README.md
```

## 6. Modelo de Dados (PostgreSQL)

```sql
-- Diretório raiz configurado pelo usuário
CREATE TABLE library_roots (
    id SERIAL PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Coleções (série, ou filme avulso) derivadas da estrutura de pastas
CREATE TABLE collections (
    id SERIAL PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    folder_path TEXT NOT NULL UNIQUE,
    type TEXT NOT NULL DEFAULT 'series', -- 'series' | 'movie'
    poster_path TEXT,                    -- preenchido futuramente (TMDB), opcional
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Temporadas (subpastas dentro de uma coleção do tipo 'series')
CREATE TABLE seasons (
    id SERIAL PRIMARY KEY,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    folder_path TEXT NOT NULL UNIQUE,
    season_number INTEGER
);

-- Arquivo de vídeo individual (episódio ou filme)
CREATE TABLE videos (
    id SERIAL PRIMARY KEY,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    season_id INTEGER REFERENCES seasons(id) ON DELETE CASCADE, -- NULL se for filme avulso
    file_path TEXT NOT NULL UNIQUE,      -- identificador principal do arquivo
    file_hash TEXT,                      -- opcional: hash parcial para detectar arquivo movido/renomeado
    display_name TEXT NOT NULL,
    episode_number INTEGER,
    duration_seconds DOUBLE PRECISION,   -- preenchido após primeira leitura de metadata via libVLC
    added_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Progresso de reprodução por vídeo
CREATE TABLE playback_progress (
    video_id INTEGER PRIMARY KEY REFERENCES videos(id) ON DELETE CASCADE,
    position_seconds DOUBLE PRECISION NOT NULL DEFAULT 0,
    watched BOOLEAN NOT NULL DEFAULT FALSE,
    last_played_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_videos_collection ON videos(collection_id);
CREATE INDEX idx_videos_season ON videos(season_id);

-- Intervalo de abertura/intro por coleção ou por temporada
-- (aberturas costumam ter o mesmo tempo de início/fim em todos os episódios de uma temporada)
CREATE TABLE intro_markers (
    id SERIAL PRIMARY KEY,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    season_id INTEGER REFERENCES seasons(id) ON DELETE CASCADE, -- NULL = aplica a toda a coleção
    start_seconds DOUBLE PRECISION NOT NULL,
    end_seconds DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (collection_id, season_id)
);

-- Preferência do usuário (única linha, tabela de configuração global)
CREATE TABLE user_settings (
    id SERIAL PRIMARY KEY,
    auto_skip_intro BOOLEAN NOT NULL DEFAULT FALSE
);
```

**Regra de negócio "assistido":** marcar `watched = true` automaticamente quando `position_seconds >= 0.9 * duration_seconds`. Não marcar antes disso, mesmo que o usuário feche o player.

**Identificador do vídeo:** usar `file_path` como chave de correspondência primária. `file_hash` é opcional e serve apenas para tentar recuperar o progresso caso um arquivo seja movido/renomeado dentro da mesma raiz — não é obrigatório na primeira versão.

## 7. Scanner de Biblioteca

Responsável por: dado um `root_path`, percorrer recursivamente e popular `collections`, `seasons`, `videos`.

Regras:

1. Extensões de vídeo aceitas (config expansível): `.mkv`, `.mp4`, `.avi`, `.mov`, `.wmv`, `.m4v`, `.webm`.
2. Pasta de primeiro nível dentro da raiz = uma `collection`.
3. Se a `collection` tiver subpastas contendo vídeos = tipo `series`, cada subpasta vira uma `season`.
4. Se a `collection` tiver vídeos soltos direto na pasta, sem subpastas = tipo `movie` (ou uma "temporada única" implícita, a critério do agente — mas manter consistência).
5. **Ordenação:** usar _natural sort_ (não ordenação lexicográfica pura) para nomes de arquivos e pastas, para que `Ep2` venha antes de `Ep10`. Implementar em `scan/natural_sort.rs` ou usar crate `natord`.
6. Scan deve ser incremental: ao rodar de novo sobre a mesma raiz, não duplicar registros já existentes (`UPSERT` por `file_path`/`folder_path`), e detectar arquivos removidos do disco (marcar como órfãos ou remover — decisão do agente, mas não deixar quebrar a UI).
7. Rodar o scan em paralelo (crate `rayon` ou tasks assíncronas com `tokio`) quando a árvore de diretórios for grande — esse é o ponto de "processamento pesado" do projeto.

## 8. Engine de Reprodução (libVLC)

- Usar `vlc-rs` (bindings de `vlc-rs`/`libvlc-sys`) para instanciar o player.
- O vídeo deve renderizar embutido na janela da aplicação (usar o handle nativo da janela Tauri — `tauri::WebviewWindow` expõe o HWND no Windows via `window.hwnd()` ou equivalente na versão do Tauri usada) e não abrir uma janela separada do VLC.
- Comandos expostos como `tauri::command` para o frontend:
  - `play_video(video_id)`
  - `pause()`
  - `seek(position_seconds)`
  - `get_current_position() -> f64`
  - `get_duration() -> f64`
- A cada intervalo curto (sugestão: 5 segundos, via timer no backend ou polling do frontend), salvar `position_seconds` no banco (`UPSERT` em `playback_progress`). Usar throttle — não escrever a cada frame.
- Ao pausar, trocar de vídeo, ou fechar o app, forçar um salvamento final da posição (não confiar só no polling periódico).
- Ao abrir um vídeo que já tem `playback_progress`, dar `seek` automático para `position_seconds` salvo antes de iniciar a exibição visual (evitar mostrar o frame inicial por um instante antes do seek).

## 9. Pular Abertura (Skip Intro)

Não é detecção automática por análise de áudio/vídeo (isso é um projeto à parte, fora de escopo — ver seção 10). O mecanismo é baseado em **marcação de intervalo**, reaproveitável entre episódios da mesma temporada, funcionando assim:

1. O usuário marca manualmente, em qualquer episódio, o `start_seconds` e `end_seconds` da abertura (ex: dois botões no player — "marcar início da abertura" e "marcar fim da abertura" — ou dois campos numéricos na tela de detalhe da coleção).
2. Esse marcador é salvo em `intro_markers`, vinculado à `season_id` (se aberturas variam por temporada) ou só à `collection_id` (se for igual em todos os episódios da coleção inteira).
3. Ao reproduzir qualquer episódio dessa temporada/coleção, o backend consulta `intro_markers` correspondente e informa o intervalo ao frontend.
4. Durante a reprodução, quando `position_seconds` estiver dentro de `[start_seconds, end_seconds]`:
   - Se `user_settings.auto_skip_intro = true`: o backend já faz `seek(end_seconds)` automaticamente.
   - Se for `false`: o frontend exibe um botão flutuante **"Pular abertura"** sobre o vídeo; ao clicar, chama `seek(end_seconds)`.
5. O botão flutuante desaparece automaticamente assim que `position_seconds > end_seconds`.
6. Adicionar um toggle nas configurações do app para ligar/desligar `auto_skip_intro` (persistido em `user_settings`).

Comando novo a expor via `tauri::command`:

- `get_intro_marker(video_id) -> Option<{ start_seconds, end_seconds }>` — resolve automaticamente a prioridade `season_id` > `collection_id`.
- `set_intro_marker(collection_id, season_id: Option<i32>, start_seconds, end_seconds)`
- `set_auto_skip_intro(enabled: bool)`

## 10. Frontend — Telas Mínimas (MVP)

1. **Configuração inicial:** seletor de diretório raiz (usar API de diálogo nativo do Tauri, `@tauri-apps/plugin-dialog`), dispara o scan.
2. **Biblioteca:** grid de `collections`, com indicação visual de progresso (ex: barra fina embaixo do card) e badge de "não assistido" quando houver episódios novos.
3. **Detalhe da coleção:** lista de temporadas/episódios com natural sort aplicado, indicando assistido/não assistido/em progresso.
4. **Player:** área de renderização do vídeo (controlada pelo libVLC embutido) + controles de play/pause/seek/volume + botão de próximo episódio.
5. **Continue assistindo:** seção na tela inicial listando os últimos vídeos com `playback_progress.watched = false AND position_seconds > 0`, ordenados por `last_played_at DESC`.
6. **Configurações:** toggle de `auto_skip_intro` e, na tela de detalhe da coleção/temporada, campos/botões para marcar `start_seconds`/`end_seconds` da abertura (ver seção 9).

## 11. Fora de Escopo do MVP (não implementar agora)

- Detecção **automática** de abertura por análise de áudio/vídeo (comparação de fingerprint entre episódios) — o skip intro do MVP é por marcação manual de intervalo (seção 9), não detecção.
- Integração com TMDB/metadata externo para pôsteres e sinopses.
- Suporte a legendas externas (.srt) — pode ser adicionado depois, libVLC já suporta nativamente quando chegar a hora.
- Multiplataforma (macOS/Linux) — foco é Windows.
- Sincronização de progresso entre múltiplos dispositivos.
- Empacotamento do libVLC junto ao instalador (por ora depende do VLC já instalado na máquina do usuário).

## 12. Ordem Sugerida de Implementação

1. Setup do projeto Tauri + conexão com PostgreSQL (`sqlx`) + rodar migrations do schema acima.
2. Scanner de pastas com natural sort, populando o banco a partir de um diretório de teste.
3. Tela de biblioteca (grid) consumindo os dados do scan.
4. Integração libVLC — reproduzir um vídeo embutido na janela, comandos básicos de play/pause/seek.
5. Persistência de progresso (polling + salvamento final) e regra de "assistido" automática.
6. Seção "Continue assistindo" na home.
7. Skip intro: marcação manual de intervalo + botão flutuante + auto-skip opcional.
8. Polimento de UI e tratamento de erros (arquivo removido do disco, banco indisponível, etc.).
