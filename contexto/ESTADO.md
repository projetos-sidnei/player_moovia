# ESTADO — player_moovia

> Atualizar este arquivo ao fim de CADA etapa. Máx ~40 linhas — cortar histórico velho, manter só o vivo.

## Fase atual

**Fases 1–7 + fase 8 em andamento** (iterando com feedback de teste do usuário). Correção de cursos (2026-09-12): tipo `course` integrado ao frontend/backend; re-scan preserva coleções catalogadas manualmente como curso; código experimental quebrado de `add_course_collection` foi removido. Validado: `npm run build` ok e `cargo check -j 4 --target-dir target-course-check` ok. Última rodada (2026-08-05): fullscreen (F/duplo clique/botão), menu de áudio/legendas (via vlc::sys — ver decisoes.md), monitor sem trabalho quando pausado. Validado: `tsc` ok · `cargo build` ok. Rodada anterior (2026-08-04): fix libVLC delay-load, scanner (vídeos na raiz, SxxEyy, nomes legíveis), player redesenhado.

Validação de memória (2026-09-13): identificados cache ilimitado de frames de prévia e capturas pendentes sem deduplicação no `PlayerPage`; cache limitado a 120 entradas, requisições por bucket deduplicadas e respostas antigas invalidadas. `npm run build` e `cargo check --lib -j 4` ok. `cargo test` segue bloqueado por `examples/preview_smoke.rs`, que espera `PreviewHandle::open` retornar `Result`, embora a API atual retorne `()`.

Cursos e remoção em lote (2026-09-13): raiz `Cursos`/`cursos` agora mantém um card por pasta de curso, classifica automaticamente como `course` e agrupa vídeos de subpastas dentro das seções do curso. Home ganhou modo de seleção múltipla para remover vários cards apenas da biblioteca. Validado: teste `scan::walker`, `npm run build` e `cargo check --lib` ok.

Correção de capas de cursos (2026-09-13): geração agora limita o instante à duração conhecida e tenta o primeiro frame quando o seek solicitado não produz imagem, cobrindo aulas curtas e vídeos sem duração catalogada. `cargo check --lib` ok.

Reprocessamento de capas (2026-09-13): painel Organizar ganhou `Reprocessar outro frame`, que avança 30 segundos a cada tentativa e preserva a capa anterior até a nova captura concluir. `npm run build` ok.

Layout da Biblioteca (2026-09-13): Home passou a ocupar a altura da janela sem scroll global; nome/botoes do topo, Continue assistindo, busca e filtros ficam fixos, e somente o grid de cards possui rolagem vertical. `npm run build` ok.

Badge dos cards (2026-09-13): tipo Série/Filme/Curso passou a usar badge escuro com texto branco, borda e sombra para manter contraste sobre capas claras. `npm run build` ok.

Estabilidade dos cards (2026-09-13): badge de tipo ganhou posição/camada estáveis no rodapé esquerdo mesmo antes da capa carregar; nomes dos catálogos agora aparecem em até duas linhas e têm tooltip com o título completo. `npm run build` ok.

Correção do re-scan na Home (2026-09-13): mensagem de status passou a usar espaço fixo e não altera a altura da Biblioteca; capa e título dos cards não encolhem mais, e o nome do catálogo fica visível abaixo da imagem. `npm run build` ok.

Controle de miniaturas por catálogo (2026-09-13): migration 0005 adiciona preferência persistente para mostrar/ocultar miniaturas; painel Organizar oferece processar faltantes ou reprocessar todas; cada episódio permite gerar/reprocessar individualmente. Validado: `npm run build` e `cargo check --lib` ok.

Tooltips de ações (2026-09-13): botões do painel Organizar, controles de miniaturas, edição/exclusão de episódios e navegação da coleção receberam atributos `title` explicativos. `npm run build` ok.

Progresso de miniaturas (2026-09-13): geração em lote passou a emitir evento por vídeo com processados, geradas, falhas e arquivo atual; UI mostra barra, tempo decorrido e estimativa restante. `npm run build` e `cargo check --lib` ok.

Cancelamento de miniaturas (2026-09-13): botão `Interromper` solicita cancelamento cooperativo, interrompe o lote entre capturas e preserva as miniaturas já concluídas; UI informa o estado interrompido. `npm run build` e `cargo check --lib` ok.

Barra de progresso visível (2026-09-13): componente foi movido para o bloco correto de Miniaturas e aparece desde `Preparando processamento`, sem depender de `total > 0`; `npm run build` ok.

Fallback de captura de miniaturas (2026-09-13): o capturador agora limita o instante pela duração real detectada pelo libVLC, evitando seek em 300s além do fim de aulas curtas; o fallback para o primeiro frame permanece ativo. O texto `split ...` no progresso é apenas o nome do vídeo atual, não um erro. `cargo check --lib` ok.

Detecção local de introdução (2026-09-13): implementada primeira versão somente para séries. Usuário seleciona episódios, limita minutos iniciais e intervalo de amostragem; o backend compara hashes perceptuais de frames em baixa resolução, emite progresso/cancelamento e retorna sugestões por episódio com confiança. Aceite grava marcador por vídeo em migration 0006; marcadores manuais de temporada/coleção continuam como fallback e não são sobrescritos. `npm run build` e `cargo check --lib` ok. Ainda requer teste com episódios reais e é heurística visual, sem áudio/IA.

As sugestões de introdução agora permitem editar início/fim antes do aceite individual. `npm run build` ok.

UX da análise de introdução (2026-09-13): seleção com menos de dois episódios exibe aviso explícito e bloqueia o processamento; cada episódio concluído recebe marca visual `analisado`; o painel informa conclusão, ausência de resultados ou interrupção sem confundir os estados. `npm run build` e `cargo check --lib` ok.

## Ordem das fases (spec §12)

1–7. ✔ · 8 (polimento) em andamento conforme feedback de teste do usuário.

## Próxima ação concreta

- Próxima rodada de UX: adicionar na página inicial uma ação para remover uma pasta da biblioteca e uma seleção de vídeos para exclusão em lote, sempre com confirmação e distinção entre remover do player e enviar arquivos para a Lixeira.
- Depois: desenhar e implementar um método inteligente para detectar o intervalo da introdução e oferecer a opção de pular, sem substituir imediatamente os marcadores manuais existentes.
- Usuário ainda deve testar: **áudio** (causa achada: prévia mutava a sessão do processo — ver decisoes.md), barras pretas de filmes, e o painel **⚙ Organizar** (catalogação série/filme, agrupar em outra coleção, nomes, renomear, capa, excluir).
- Confirmado em testes reais: vídeo renderiza ✔ · controles ✔ · 2 janelas ✔ · captura de frame ✔ · migrations 1–3 aplicam ✔ · parser testado (3 testes unitários) ✔.
- Pendências conhecidas: preferência de trilha só na sessão (não persiste entre execuções); skip intro segue com marcador único por temporada (ver "Decisões em aberto"). `cargo fmt --check` ainda acusa formatação preexistente em vários arquivos.
- Implementado em 2026-08-05 (8 melhorias): próximo episódio automático, assistido manual, busca/filtros, legendas externas .srt, sincronia áudio/legenda, velocidade, miniaturas por episódio, remover pasta da biblioteca.

## Referências para a fase atual

`player.md` + `frontend.md` (+ `decisoes.md` seção 2026-08-04 fases 2–7)

## Ambiente (verificado em 2026-08-04)

- Rust 1.93.0 · Node v24.11.0 · VLC 64-bit em `C:\Program Files\VideoLAN\VLC` · PostgreSQL 17 (psql fora do PATH: `C:\Program Files\PostgreSQL\17\bin\psql.exe`)
- Banco `player_moovia` criado; migrations rodam no startup do app.
- **Compilar sempre com `-j 4` e sem sandbox** — paralelismo padrão derruba o rustc (STATUS_STACK_BUFFER_OVERRUN). Já fixado em `src-tauri/.cargo/config.toml`.
- Release: `npm run tauri build` → exe + instaladores NSIS/MSI (ver decisoes.md e README).
- `vlc.lib` em `src-tauri/vlc-lib/` (gerada localmente, fora do git); DLLs do VLC copiadas em `target/debug/`.

## Pendências / avisos

- Primeiro commit git ainda não feito (fazer quando o usuário pedir).
- Empacotamento release: copiar DLLs do VLC para o dir do exe (hoje só em target/debug).
