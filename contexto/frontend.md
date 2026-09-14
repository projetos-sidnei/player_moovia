# Frontend — telas MVP (spec §10) e limites (spec §11)

Vite + TypeScript. Framework: React ou Svelte (registrar escolha em decisoes.md). Comunicação: `invoke` + eventos `emit`/`listen` (progresso em tempo real).

## Telas

1. **Configuração inicial:** seletor de diretório raiz (`@tauri-apps/plugin-dialog`) → dispara scan.
2. **Biblioteca:** grid de collections; barra fina de progresso no card; badge "não assistido" p/ episódios novos.
3. **Detalhe da coleção:** temporadas/episódios (natural sort), estado assistido/em progresso/não assistido.
4. **Player:** área do vídeo (libVLC embutido) + play/pause/seek/volume + próximo episódio.
5. **Continue assistindo (home):** `watched=false AND position_seconds>0`, ordem `last_played_at DESC`.
6. **Configurações:** toggle `auto_skip_intro`; na tela da coleção/temporada, campos/botões p/ marcar intro.

## Fora de escopo do MVP (NÃO implementar)

- Detecção automática de abertura (fingerprint A/V).
- TMDB / pôsteres / sinopses.
- Legendas .srt externas.
- macOS/Linux (foco Windows).
- Sync multi-dispositivo.
- Empacotar libVLC no instalador (depende do VLC instalado).
