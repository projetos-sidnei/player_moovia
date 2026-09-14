# Log de decisões (append-only)

> Registrar aqui toda decisão de implementação que NÃO está na spec, com data e motivo curto. Consultar antes de re-decidir qualquer coisa.

## 2026-08-04 — Sistema de contexto criado

- `contexto/` substitui releituras de `docs/projeto.md`. Entrada única por sessão: `ESTADO.md`; referências por domínio carregadas sob demanda (protocolo no `CLAUDE.md` da raiz).

## 2026-08-04 — Fase 1 (setup)

- **Frontend: React 19 + TypeScript** (template oficial `create-tauri-app --template react-ts`) — suporte mais maduro no ecossistema Tauri.
- Crate lib Rust chama `player_moovia_lib` (padrão Tauri Windows para evitar conflito bin/lib).
- sqlx 0.8 com `runtime-tokio` + `tls-rustls` + `chrono` + `migrate`; migrations em `src-tauri/migrations/` aplicadas no startup (`db::init_pool`).
- `user_settings` já nasce com a linha default (`auto_skip_intro=false`) inserida na migration 0001 — código lê sempre a linha única, nunca precisa criar.
- Banco `player_moovia` criado manualmente via psql em 2026-08-04 (CREATE DATABASE não entra em migration).
- Pool: 5 conexões máx (`DB_MAX_CONNECTIONS` em constants.rs).

## 2026-08-04 — Fases 2–7 (implementação completa)

**Scanner (fase 2):**

- Filme avulso = tipo `movie` com `season_id NULL` (sem temporada implícita). Coleção mista (subpastas + vídeos soltos) = `series` com vídeos soltos em `season_id NULL`.
- Arquivo removido do disco = **DELETE** do registro (cascade apaga progresso) + limpeza de seasons/collections vazias. Sem estado "órfão".
- `episode_number`/`season_number` = primeiro grupo de dígitos do nome; `None` se não houver (UI ordena por natural sort do nome, não pelo número).
- Paralelismo: `spawn_blocking` (IO-bound); sem rayon.
- Natural sort via crate `natord` (`scan/natural_sort.rs` é só um wrapper).

**libVLC (fase 4):**

- Módulo Rust chama `vlc_engine/` (não `vlc/` como na spec) para não colidir com o crate `vlc`.
- Objetos libVLC não são `Send` → thread dedicada dona de Instance/MediaPlayer; app fala com ela via `mpsc` + respostas `oneshot` (`PlayerHandle` é o lado Send/Sync injetado como State).
- **Import lib:** VLC instalado não tem SDK → `vlc.lib` gerada de `libvlc.dll` (dumpbin /exports → .def → lib.exe) em `src-tauri/vlc-lib/` (fora do git; regenerar com os mesmos passos se sumir). `build.rs` adiciona o link-search. `libvlc.dll`+`libvlccore.dll` copiadas para `target/debug/`.
- `VLC_PLUGIN_PATH` setado no startup para `<VLC_DIR>\plugins`; `VLC_DIR` opcional no .env (default `C:\Program Files\VideoLAN\VLC`).
- Render: HWND da janela principal + janela `transparent: true` + `body.player-mode { background: transparent }` no CSS — vídeo desenha atrás da webview e aparece onde o HTML é transparente.

**Progresso/skip intro (fases 5–7):**

- Monitor por reprodução: task tokio com tick de 1s (`PLAYER_TICK_MS`) — emite evento `playback-progress`, auto-skip de intro no backend, persiste a cada 5 ticks (`PROGRESS_SAVE_INTERVAL_SECS`). Salvamento final forçado em pause/troca/stop/fechar janela.
- `auto_skip_intro` cacheado em `SettingsCache` (AtomicBool) — sem query por tick.
- Rever vídeo com posição >= duração-1s recomeça do zero.
- `intro_markers`: UNIQUE **NULLS NOT DISTINCT** (PG15+; linter SQL do IDE reclama, ignorar) para upsert de marker de coleção inteira (season_id NULL).
- Marcação de intro: no player (botão 2 estágios início/fim) e na tela da coleção (campos numéricos por temporada).

**Frontend (fases 3/6):**

- Sem router: união discriminada `Screen` em `navigation.ts` + estado no App.
- Todo invoke passa por `src/api.ts` (única camada); nomes em `ipc.constants.ts`; eventos em `events.constants.ts`.
- Sem StrictMode (double-mount de dev dispararia play/stop duplo no VLC).
- Frontend faz o botão "Pular abertura" via posição do evento de progresso; auto-skip é do backend.

## 2026-08-04 — Correções pós-teste do usuário

- **libVLC no Windows: NUNCA usar `std::env::set_var("VLC_PLUGIN_PATH")`** — a CRT da libvlc.dll captura o environment no start do processo e não vê set_var; `libvlc_new` retorna null. Solução definitiva: **delay-load** (`/DELAYLOAD:libvlc.dll` + `delayimp.lib` no build.rs) + `SetDllDirectoryW(vlc_dir)` no início do `run()` — a DLL vem da instalação do VLC e os plugins são achados relativo à libvlccore.dll. Sem cópia de DLLs, sem env var. Diagnóstico reproduzível: `cargo run --example vlc_smoke`.
- Thread do VLC não morre mais em falha de init: fica viva devolvendo a mensagem de erro real em todo comando (antes: "thread do VLC encerrou" sem causa).
- **Scanner: vídeos soltos direto na raiz** agora viram uma collection com o nome da pasta raiz (caso comum: usuário seleciona a própria pasta da série). `is_series` = mais de 1 vídeo.
- `episode_number`: regex `SxxEyy` > `Ep N` > primeiro número (o antigo "primeiro número" pegava o 01 de S01 para todos os episódios).
- `display_name` legível: `.`/`_` de nomes de release viram espaço ("BLEACH.Thousand-Year..." → "BLEACH Thousand-Year ...").
- Player redesenhado: controles centrais (↺10 · play/pause grande · ↻10), auto-hide após 3s (`CONTROLS_HIDE_MS`), clique na área central pausa, atalhos (espaço/setas/Esc), seek bar com tempos nas pontas, botões ghost com blur.

## 2026-08-05 — Fullscreen, trilhas de áudio/legenda, higiene do monitor

- **Trilhas de áudio/legenda:** vlc-rs 0.3 não expõe set_track/spu e o `get_audio_track_description` dele tem off-by-one (perde a última trilha). Implementado direto via `vlc::sys` (público) + `MediaPlayer::raw()`: `collect_tracks` percorre e LIBERA a lista (`libvlc_track_description_list_release`). Comandos: `get_tracks` (retorna `TrackMenu {audio, audioCurrent, subtitles, subtitleCurrent}`), `set_audio_track`, `set_subtitle_track`. id -1 = desativado (o próprio libvlc lista "Disable" como primeira opção).
- **Fullscreen:** frontend via `getCurrentWindow().setFullscreen()` (permissões `core:window:allow-set-fullscreen`/`allow-is-fullscreen` no capabilities). Atalho F, duplo clique no vídeo, Esc sai do fullscreen antes de voltar; unmount desliga fullscreen.
- **Higiene do monitor (memória/trabalho):** tick só trabalha se a posição mudou (>0,01s) — pausado ou mídia encerrada não gera evento, query nem UPSERT. Thread do VLC fica bloqueada em `recv` quando ociosa (zero CPU); Media é refcounted e liberado na troca.
- Clique simples no centro pausa com delay de 250ms para não conflitar com o duplo clique de fullscreen.

## 2026-08-05 — ARQUITETURA DO PLAYER: duas janelas (vídeo + overlay)

**Limitação central do WebView2 (não contornável):** a webview NÃO compõe com janelas nativas atrás dela. Comprovado nos dois testes:

- Vídeo ACIMA da webview → vídeo aparece, mas cobre os controles HTML e engole o mouse.
- Vídeo ABAIXO da webview → recortado, não desenha nada (os pixels "transparentes" da webview revelam o DESKTOP, não o vídeo).

**Solução final (a que está no código):**

1. `vlc_engine/host.rs` — janela filha nativa (STATIC/SS_BLACKRECT) do vídeo, **acima** da webview, criada oculta; `set_video_host_visible` mostra só durante a reprodução (senão cobriria a biblioteca).
2. Controles do player vivem numa **segunda janela Tauri** (`player-overlay`): `transparent + decorations(false) + shadow(false) + skip_taskbar + parent(main)`. Janelas de topo SÃO compostas entre si pelo DWM → controles aparecem sobre o vídeo e recebem o mouse. Sem `always_on_top`: `parent()` (owner no Windows) já a mantém acima da principal sem flutuar sobre outros apps.
3. Geometria: `WindowEvent::Resized|Moved` da principal → `sync_player_surfaces` (redimensiona o vídeo + cola o overlay no client area via `inner_position`/`inner_size`).
4. Fullscreen aplica-se à janela PRINCIPAL (dona do vídeo) via comando `set_player_fullscreen` — fullscreen no overlay sozinho não afetaria o vídeo.
5. Ciclo: `open_player` (mostra vídeo + cria overlay) → `close_player` (salvamento final, para o VLC, esconde vídeo, destrói overlay, emite `player-closed` → a biblioteca remonta e recarrega o progresso).

**Frontend:** `main.tsx` decide a raiz pelo **label da janela** (`getCurrentWindow().label === "player-overlay"`), não pela query string — se a detecção falhasse, a biblioteca renderizaria opaca cobrindo o vídeo. videoId/collectionId vêm da query (`Url::join` do Tauri preserva query string). `Screen` não tem mais variante "player"; `Navigator.play()` chama `open_player`. **Capabilities:** a janela `player-overlay` PRECISA estar em `windows` do capability, senão o JS dela não invoca nada.

## 2026-08-05 — Trilha lembrada, prévia no hover, playlist

**Preferência de trilha (áudio/legenda) entre vídeos:**

- Ids de trilha do libVLC são POR ARQUIVO — guardar o id não funciona. A escolha é guardada pelo **nome** da trilha (`TrackChoice::{Disabled, Named}` em `commands/player.rs`, state `TrackPrefs`) e resolvida por nome no próximo vídeo (match exato → case-insensitive → senão mantém o padrão).
- Reaplicação: as trilhas só ficam listáveis alguns instantes após o play, então o monitor tenta reaplicar a cada tick até conseguir (limite `TRACK_APPLY_MAX_ATTEMPTS`).
- Escopo: sessão do app (não persiste em banco). Se pedirem persistência, virar coluna em `user_settings` ou tabela por coleção.

**Prévia no hover da barra (`vlc_engine/preview.rs`):**

- VLC 3.x não tem `libvlc_media_thumbnail_request_*` (só 4.x). Solução: SEGUNDO MediaPlayer **sem janela** com `libvlc_video_set_callbacks` + `libvlc_video_set_format("RV32")` → o libVLC decodifica direto num buffer nosso; seek + captura sem tocar na reprodução principal.
- Validado headless com arquivo real (`cargo run --example preview_smoke -- <arquivo> <segundos> [dir]`): frame correto, ~890ms o primeiro, ~270ms os seguintes. O exemplo salva PNG para conferência visual.
- Frame → PNG (crate `image`) → data URI base64 (evita configurar asset protocol/scope).
- **Memória:** a instância libVLC da prévia é criada só no PRIMEIRO hover e destruída no `close_player`. Quem não usa a prévia não paga nada.
- Frontend: debounce 180ms, cache por faixa de 5s (`PREVIEW_BUCKET_SECONDS`), cache limpo ao trocar de vídeo; a resposta só é aplicada se o mouse ainda estiver na mesma faixa.
- Sincronização do buffer: `FrameSink` com `UnsafeCell` + contador atômico de frames; a leitura só ocorre com o player pausado (ver comentários SAFETY).

**Playlist:** painel lateral direito no player (botão "☰ Episódios"), reusa a lista já carregada de `get_collection_detail`; item atual destacado, clique troca de episódio. **Auto-hide:** controles ficam fixos enquanto pausado ou com painel (trilhas/playlist) aberto.

## 2026-08-05 — Organização manual da coleção (título, renomear, capa)

Tudo MANUAL por decisão do usuário — nenhuma dessas ações roda no scan nem automaticamente.

- **Migration 0002**: `collections.title_locked`. O upsert do scanner passou a fazer `title = CASE WHEN title_locked THEN title ELSE EXCLUDED.title END` — sem isso, o re-scan sobrescreveria o nome digitado à mão com o nome da pasta.
- **Renomear em lote** (`commands/organize.rs`): padrão com marcadores `{serie} {temporada} {episodio} {titulo}` (constantes em constants.rs / organize.constants.ts). Fluxo obrigatório: `preview_rename` (mostra de → para, marca conflitos) → `apply_rename`. O apply **recalcula o plano no backend** (não confia na lista do cliente), pula itens em conflito ou sem mudança, faz `fs::rename` e atualiza `videos.file_path`/`display_name` (o progresso segue pelo video_id).
  - Numeração: usa `episode_number` do arquivo; se faltar, a posição em ordem natural dentro da temporada.
  - Sanitização dos caracteres proibidos do Windows antes de montar o nome.
- **Capa**: reaproveita o capturador de frames (`preview.rs`) — refatorado em `Grabber` parametrizado por resolução: prévia do hover em 256x144, capa em 640x360 com instância temporária descartada após a captura. PNG gravado em `app_data_dir()/posters/{id}.png`, caminho na coluna `poster_path` (que a spec já reservava para TMDB). A UI lê via comando que devolve data URI (evita configurar asset protocol/scope); o card só busca a capa se `has_poster` for true.
- **Decisão de escopo**: renomear altera arquivos do usuário no disco → sempre com pré-visualização e confirmação explícita; nunca sobrescreve arquivo existente.

## 2026-08-05 — Nome do episódio: parser + edição manual

- **Parser** em `scan/release_name.rs` (com testes unitários — `cargo test --lib release_name`): o nome do episódio é o que está ENTRE o marcador `SxxEyy` e a primeira tag técnica (resolução `\d{3,4}p`, ano, fonte WEB-DL/BluRay, distribuidora DSNP/AMZN, codec x264/HEVC, áudio AAC/DDP, DUAL, grupo após hífen — lista `RELEASE_TOKENS`). Depois normaliza para Title Case.
  - `BLEACH.Thousand-Year.Blood.War.S01E01.THE.BLOOD.WARFARE.1080p.DSNP.WEB-DL` → `The Blood Warfare`.
  - Sem nome identificável (ex. `O.Rastreador.S01E02.1080p...`) devolve None — cai para edição manual.
  - `walker.rs` passou a usar esse módulo (regexes e `prettify` saíram de lá).
- **Migration 0003**: `videos.display_name_locked`. Nome definido pelo usuário (extraído OU digitado) fica travado contra o re-scan **e** contra o `apply_rename` (que antes reescrevia display_name com o novo stem).
- **Fluxo**: "Pré-visualizar nomes" → "Aplicar" no painel Organizar (não toca nos travados); exceções via lápis ✎ na lista de episódios.
- **Marcador `{nome}`** na renomeação = nome do episódio (travado do usuário > parser > vazio); `{titulo}` continua sendo o stem bruto. Padrão default mudou para `S{temporada}E{episodio} - {nome}`. `render_pattern` apara hífen/underscore sobrando quando `{nome}` vem vazio.

## 2026-08-05 — Exclusão (do player e do PC)

- Dois níveis, por episódio e por coleção (`commands/removal.rs`):
  - **"Só do player"**: DELETE dos registros (CASCADE leva progresso/temporadas/marcadores). Arquivos ficam no disco — um novo scan traz de volta.
  - **"Excluir do PC"**: manda para a **Lixeira do Windows** (crate `trash`), não `fs::remove_file` — exclusão destrutiva sem desfazer é inaceitável num clique. Depois remove os registros.
- **Regra de consistência**: na coleção, se qualquer arquivo falhar ao ir para a Lixeira, NADA é removido do banco (senão a biblioteca ficaria fora de sincronia com o que sobrou no disco). Erros voltam para a UI.
- `stop_if_playing` (em `player.rs`) para a reprodução e libera a prévia se o vídeo alvo estiver tocando — o Windows não deixa mexer em arquivo aberto pelo VLC.
- Pastas que ficam vazias são removidas (da mais profunda para a mais rasa, com `remove_dir` que falha sozinho se ainda houver algo dentro) — nunca `remove_dir_all`, para não levar junto legendas/extras do usuário.
- UI: confirmação sempre em dois passos, na própria linha do episódio ou no bloco "Excluir coleção" do painel Organizar. Nunca há exclusão em um clique só.

## 2026-08-05 — Agrupar temporadas quando a raiz é a própria série

- **Sintoma real:** raiz apontada para `G:\Series\O Rastreador` (pasta da série). Pela regra "1º nível = coleção", `T1` e `T2` viraram DUAS coleções, e as pastas de release dentro delas viraram as temporadas.
- **Correção:** se todas as pastas de 1º nível da raiz forem nomes de temporada (`is_season_folder`: T1, S02, Season 3, Parte 2…), a RAIZ vira uma coleção única e essas pastas viram as temporadas. Busca de vídeos na temporada é recursiva, então a pasta de release aninhada não atrapalha.
- Raiz de biblioteca (pastas com nome de série) continua igual — coberto por teste (`scan::walker::tests`).
- **Migração ao re-escanear:** os vídeos são UPSERT por `file_path`, então mudam de coleção/temporada preservando progresso (é o `video_id` que importa). As coleções antigas ficam sem vídeos e o `prune_missing` as remove. Perde-se o título manual e os marcadores de abertura ligados às coleções antigas.

## 2026-08-05 — Correções do teste com filme em 2.39:1

- **Janela principal era `transparent: true`** (sobra da 1ª arquitetura). Com o vídeo em cinemascope, as barras pretas ficavam com alpha e deixavam ver a tela da coleção por trás (header "Voltar/Organizar" aparente). Só o OVERLAY deve ser transparente; a principal agora é opaca. Não reativar.
- **Painéis do player não usam mais `backdrop-filter`** e têm fundo opaco (`#12121a`): sobre janela transparente o blur amostrava o conteúdo da janela de baixo (lista de episódios) e embaralhava o texto das trilhas/legendas.
- **Bug do áudio mudo:** `TrackChoice::from_selection` caía em `Disabled` quando a trilha não era identificável por nome — a preferência então DESLIGAVA o áudio nos vídeos seguintes. Agora devolve `Option` (trilha desconhecida = esquece a preferência) e **áudio nunca guarda `Disabled`** (vídeo mudo não é preferência que se arraste). Legenda desligada continua persistindo, que é intencional.

## 2026-08-05 — CAUSA DO ÁUDIO MUDO (investigação longa, não repetir)

**Sintoma:** sem áudio no player; VLC e testes isolados tocavam normal. **Causa:** `Grabber::new` (prévia) chamava `mdp.set_mute(true)`. No Windows o libVLC aplica mute na **sessão de áudio do PROCESSO** (ISimpleAudioVolume), compartilhada por todos os players do app → a prévia mutava a reprodução principal. Bastava passar o mouse na barra uma vez. **Correção:** a prévia nunca usa `set_mute`; usa `libvlc_audio_output_set(mdp, "adummy")` + `libvlc_audio_set_track(mdp, -1)` — nem encosta no dispositivo de som. **Nunca chamar `set_mute` num player secundário.**

O que a investigação descartou (não refazer):

- Plugins/codec/instalação do VLC: ok (eac3 6ch 48kHz lido certo; VLC.exe toca).
- `vlcrc` do usuário: limpo.
- Módulos de saída (mmdevice/directsound/waveout): todos ok.
- "failed to create audio output" no log **é normal** — é o VLC tentando passthrough e caindo para o `avcodec`; aparece igual no VLC.exe funcionando.
- Sonda com `set_hwnd` numa janela sem message loop trava a reprodução inteira (pos fica em 0) — artefato do teste, não bug do app.

## 2026-08-05 — Barras pretas de filmes e catalogação manual

- **Barras (cinemascope):** o que aparece nelas é a janela principal, atrás da janela do vídeo. Além de a principal ser opaca, ela agora renderiza `.playing-backdrop` (preto) enquanto o player está aberto — nada de UI vaza para as barras.
- **Catalogação manual** (painel Organizar): `set_collection_type` (série ↔ filme) e `merge_collection_into` — a coleção vira uma temporada de outra (temporadas migram; vídeos soltos viram uma temporada com o título da origem; a origem é apagada). Tudo numa transação.

## 2026-08-05 — Autoteste de áudio dentro do app + temporada pelo marcador

- **`MOOVIA_SELFTEST=1`** no executável: toca o primeiro vídeo da biblioteca pelo caminho REAL (janela do vídeo + engine) e imprime `[selftest]` com tocando/posição/trilha/volume/mute a cada segundo, depois sai. Criado porque o áudio só falhava dentro do app; testes isolados sempre funcionavam. **Manter** — é o jeito rápido de obter fatos do processo real sem depender da UI.
- Resultado do autoteste após as correções: `tocando=true`, posição avançando, `trilha=1` de 3, `volume=80`, `mute=0` — libVLC configurado corretamente.
- Além do `adummy` na prévia, o `Play` agora força `libvlc_audio_set_mute(mp, 0)`: o Windows **lembra o mute por aplicativo entre execuções**, então máquinas afetadas pelo bug antigo continuariam mudas mesmo com o código corrigido.
- **`season_number` pelo marcador** (`release_name::season_number`): `S01`/`T2`/`Season 17` antes do primeiro número — antes `O.Rastreador.2024.S01` virava temporada **2024** (e a renomeação gerou `S2024E01`). Rodar a renomeação de novo corrige os nomes já gravados.

## 2026-08-05 — Oito melhorias de usabilidade

1. **Próximo episódio automático:** o monitor detecta `State::Ended`, salva o progresso final (marca assistido), emite `playback-ended` e encerra. O overlay mostra o cartão "A seguir" com contagem de 10s (`AUTO_NEXT_SECONDS`), com "Assistir agora" e "Cancelar". Sem próximo episódio, fecha o player.
2. **Assistido manual:** `set_video_watched(video_id, watched)` — marcar leva a posição para o fim; desmarcar **zera o progresso** (serve como "rever do início"). Botão ✓/↺ na linha do episódio.
3. **Busca e filtros** na biblioteca (client-side, `HomePage`): busca ignora acento/caixa; chips Tudo/Não assistidos/Em andamento/Séries/Filmes.
4. **Legendas externas:** ao dar play, `sibling_subtitles` procura .srt/.ass/.ssa/.sub/.vtt na mesma pasta com o mesmo prefixo do arquivo e carrega via `libvlc_video_set_subtitle_file` — elas aparecem no menu de legendas existente.
5. **Sincronia:** `set_audio_delay`/`set_subtitle_delay` (ms → µs no libVLC), com ajuste ±100ms e zerar, dentro do painel Áudio/Legenda. Zera ao trocar de vídeo.
6. **Velocidade:** `set_playback_rate` (0,75× a 2×). Mantida ao trocar de episódio, como nos streamings.
7. **Miniaturas por episódio:** reusa o `Grabber` (agora parametrizado por resolução) a 35% da duração, gravadas em `app_data/thumbs/{video_id}.png`. **Geração é manual e em lote** (botão no Organizar) — decodificar um frame por episódio ao abrir a tela seria caro e contraria o "tudo manual" pedido; a lista só carrega o que já está em cache.
8. **Remover pasta da biblioteca** nas Configurações (confirmação em 2 passos; só apaga o catálogo, não toca nos arquivos).

## 2026-08-05 — Build de release / distribuição

- `npm run tauri build` gera `target/release/player_moovia.exe` (16,4 MB) + instaladores NSIS (3,8 MB) e MSI (5,6 MB). O WiX e o NSIS são baixados pelo Tauri no primeiro build (precisa de internet).
- **`.cargo/config.toml` com `jobs = 4`** em `src-tauri/`: o paralelismo padrão derruba o rustc nesta máquina. Vale para dev e release — não remover.
- **`.env` no app instalado:** `config::load_env()` procura no diretório de trabalho e, se não achar, **ao lado do executável** — num atalho o cwd não é o da instalação. Validado rodando o exe a partir de outro diretório.
- **Erros fatais viram caixa de mensagem** (`fatal()` com MessageBoxW): no release não há console, então config inválida ou Postgres fora do ar fechavam o app sem explicação.
- O `.env` fica FORA do instalador de propósito (credenciais não são empacotadas) — é preciso copiá-lo ao lado do exe na máquina de destino. Documentado no README.
- Dependências da máquina de destino: VLC 64-bit instalado (a libvlc.dll é carregada dele em runtime) e PostgreSQL acessível com a database criada.

## Decisões em aberto

- Skip intro: aceito como está por ora (marcador único por temporada, janela exata). Se voltar ao tema, as opções discutidas foram: janela de tolerância + marcador por episódio, ou guardar a duração e pular relativo. Média dos tempos de início foi descartada (cold open de tamanho variável leva a média a um ponto sem abertura em nenhum episódio).

## 2026-09-12 — Cursos

- Curso usa a mesma hierarquia existente de coleção, temporadas e vídeos; não foram inventadas tabelas de módulos/aulas sem requisito definido.
- `course` é uma catalogação manual persistente. O scanner continua inferindo série/filme para novas coleções, mas preserva `course` quando a pasta já está catalogada como curso.
- O frontend exibe cursos separadamente, oferece filtro próprio e não mostra marcadores de abertura para eles.
- Foi removido o protótipo incompleto `add_course_collection`/`scan_course_directory`, que chamava funções inexistentes ou com assinaturas antigas e impedia a compilação Rust.

## Plano futuro — exclusão em lote e introdução inteligente

- **Exclusão de pasta na página inicial:** cada coleção deverá oferecer uma ação direta para remover a pasta da biblioteca, reaproveitando a semântica atual de remoção de catálogo; arquivos no disco não devem ser tocados nessa ação.
- **Exclusão de vídeos em lote:** a página da coleção deverá permitir selecionar vários vídeos e executar uma única confirmação para removê-los do player ou enviá-los para a Lixeira. A operação deve parar antes de alterar o banco se algum arquivo não puder ser enviado à Lixeira, mantendo a regra de consistência já usada na exclusão de coleção.
- **UX de seleção:** mostrar contador de itens selecionados, estado vazio, cancelar seleção e confirmação explícita; impedir exclusão acidental durante reprodução e atualizar progresso/listas após a operação.
- **Detecção inteligente de introdução:** investigar um método baseado em duração, similaridade de frames/áudio e comparação entre episódios da mesma coleção ou temporada para propor um intervalo, sempre exibindo pré-visualização e confiança antes de gravar.
- **Compatibilidade:** a sugestão automática não deve sobrescrever marcadores manuais; o usuário deve poder aceitar, ajustar ou rejeitar o intervalo e continuar usando o skip intro manual quando a confiança for baixa.

## 2026-09-13 — Primeira versão da detecção local de introdução

- Disponível apenas para coleções `series`; filmes e cursos não entram na análise.
- A análise é manual e limitada aos episódios selecionados, aos primeiros minutos informados e a um intervalo de amostragem configurável.
- A primeira implementação compara hashes perceptuais de frames capturados pelo `Grabber` headless em baixa resolução; não usa IA nem processamento durante a reprodução.
- O resultado é uma sugestão por episódio com confiança. O usuário aceita individualmente; só então o marcador é salvo em `video_intro_markers`.
- Marcador aceito por vídeo tem prioridade sobre marcador de temporada/coleção; marcador manual existente não é alterado automaticamente.
- O processamento é sequencial, cancelável e emite progresso. A heurística visual pode confundir cenas repetidas com abertura; revisão humana é obrigatória nesta fase.
