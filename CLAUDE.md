# player_moovia

Player desktop de biblioteca pessoal de vídeos (Windows). Tauri 2 + Rust + libVLC + PostgreSQL. Spec completa em `docs/projeto.md` (203 linhas) — **não reler a spec inteira**; usar o sistema de contexto abaixo.

## Protocolo de contexto (economia de tokens)

1. **Início de sessão:** ler apenas `contexto/ESTADO.md` (fase atual + próxima ação + qual referência carregar).
2. **Durante a tarefa:** carregar somente o(s) arquivo(s) de `contexto/` que o ESTADO.md indicar para a fase atual. Nunca carregar todos.
3. **Fim de cada etapa/sessão:** atualizar `contexto/ESTADO.md` (fase, feito, próximo passo). Decisões novas fora da spec → append em `contexto/decisoes.md`.
4. `docs/projeto.md` só deve ser consultado se houver conflito ou lacuna nos arquivos de contexto — e nesse caso, corrigir o arquivo de contexto para a próxima vez.

## Mapa de referências (`contexto/`)

| Arquivo         | Conteúdo                                              | Carregar quando          |
| --------------- | ----------------------------------------------------- | ------------------------ |
| `ESTADO.md`     | Fase atual, feito, próximo passo                      | Sempre (início)          |
| `convencoes.md` | Regras de código sem-hardcode (Rust + TS), .env       | Sempre que for codar     |
| `db.md`         | Schema SQL completo + regras de negócio dos dados     | Fases 1, 2, 5, 7         |
| `scanner.md`    | Regras do scan de pastas                              | Fase 2                   |
| `player.md`     | libVLC, comandos IPC, progresso, skip intro           | Fases 4, 5, 7            |
| `frontend.md`   | Telas MVP, convenções TS, estrutura frontend          | Fases 3, 6, 8            |
| `decisoes.md`   | Log de decisões tomadas na implementação (append-only)| Antes de decidir algo já discutido |

## Regras invioláveis (resumo mínimo)

- Nenhum literal solto: constantes centralizadas (`src-tauri/src/constants.rs` e `src/constants/`).
- Credenciais só via `.env` (dotenvy) → `struct AppConfig` → Tauri `State`. `.env` fora do git.
- Vídeo NÃO usa `<video>` HTML — libVLC renderiza no HWND da janela Tauri.
- `watched = true` quando `position_seconds >= 0.9 * duration_seconds`.
- Natural sort para nomes de arquivos/pastas (Ep2 < Ep10).
