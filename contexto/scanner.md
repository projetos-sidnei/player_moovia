# Scanner de biblioteca (spec §7)

Entrada: `root_path`. Saída: popular `collections`, `seasons`, `videos`.

## Regras

1. Extensões aceitas: `VIDEO_EXTENSIONS` em constants.rs (mkv, mp4, avi, mov, wmv, m4v, webm) — expansível.
2. Pasta de 1º nível dentro da raiz = `collection`.
3. Collection com subpastas contendo vídeos = tipo `series`; cada subpasta = `season` (a busca de vídeos na temporada é recursiva: pasta de release aninhada entra na mesma temporada).
4. Collection com vídeos soltos e sem subpastas = tipo `movie` (ou temporada única implícita — escolher e ser consistente; registrar em decisoes.md).
4b. **A RAIZ pode ser a própria série** (usuário aponta a pasta da série, não a da biblioteca), em dois casos:
   - vídeos soltos direto na raiz → coleção com o nome da pasta raiz;
   - TODAS as pastas de 1º nível são temporadas (`T1`, `S02`, `Season 3`, `Parte 2` — `release_name::is_season_folder`) → uma coleção só, com essas pastas como temporadas.
   Testes: `cargo test --lib scan::walker`.
5. **Natural sort** para arquivos e pastas (Ep2 < Ep10) — `scan/natural_sort.rs` ou crate `natord`.
6. **Incremental:** re-scan não duplica (UPSERT por `file_path`/`folder_path`); arquivos sumidos do disco → marcar órfão ou remover (decidir e registrar), sem quebrar a UI.
7. Paralelizar em árvores grandes (`rayon` ou tasks `tokio`).

## Arquivos

`scan/walker.rs` (recursão), `scan/natural_sort.rs`, comando em `commands/scanner.rs`.
