# player_moovia

Player desktop (Windows) de biblioteca pessoal de vídeos, estilo "continue assistindo":
aponte uma pasta raiz, o app escaneia séries/temporadas/episódios e filmes, reproduz com
libVLC (MKV/HEVC ok) e persiste progresso e status de assistido no PostgreSQL.

Stack: **Tauri 2 + Rust + libVLC (vlc-rs) + PostgreSQL (sqlx) + React/TypeScript (Vite)**.
Spec completa: `docs/projeto.md`. Estado e decisões de implementação: `contexto/`.

## Pré-requisitos

- Rust (MSVC) e Node.js
- VLC 64-bit instalado (default: `C:\Program Files\VideoLAN\VLC`)
- PostgreSQL local com database `player_moovia` criada (`CREATE DATABASE player_moovia;`)
- `.env` na raiz (copiar de `.env.example`)

## Setup do libVLC (uma vez por máquina)

O instalador do VLC não traz a import lib. Gerar `src-tauri/vlc-lib/vlc.lib`:

```powershell
# Prompt do MSVC (dumpbin/lib no PATH)
dumpbin /exports "C:\Program Files\VideoLAN\VLC\libvlc.dll" > exports.txt
# montar libvlc.def (LIBRARY libvlc.dll / EXPORTS / nomes) e então:
lib /def:libvlc.def /machine:x64 /out:src-tauri\vlc-lib\vlc.lib
```

A `libvlc.dll` é carregada em runtime direto da instalação do VLC (delay-load +
`SetDllDirectoryW`) — não é preciso copiar DLLs. Se o VLC estiver fora do caminho
padrão, defina `VLC_DIR` no `.env`.

## Rodar em desenvolvimento

```powershell
npm install
npm run tauri dev
```

As migrations rodam automaticamente no startup.

## Gerar o executável

```powershell
npm run tauri build
```

Saída em `src-tauri/target/release/`:

| Arquivo | Uso |
| --- | --- |
| `player_moovia.exe` | executável direto (portátil) |
| `bundle/nsis/player_moovia_<versão>_x64-setup.exe` | instalador |
| `bundle/msi/player_moovia_<versão>_x64_en-US.msi` | instalador MSI |

### O que a máquina de destino precisa

1. **VLC 64-bit instalado** — a `libvlc.dll` é carregada da instalação do VLC em runtime
   (nada é empacotado). Se estiver fora do caminho padrão, defina `VLC_DIR` no `.env`.
2. **PostgreSQL** acessível, com a database criada (`CREATE DATABASE player_moovia;`).
   As migrations rodam sozinhas no primeiro startup.
3. **Um `.env` ao lado do `player_moovia.exe`** (ou no diretório de trabalho), com:
   ```
   DATABASE_URL=postgres://usuario:senha@localhost:5432/player_moovia
   ```
   Sem ele o app abre uma caixa de erro explicando o que falta, em vez de fechar calado.

> O `.env` fica fora do instalador de propósito: credenciais não são versionadas nem
> empacotadas. Copie o `.env.example` e preencha na máquina de destino.

## Diagnóstico

`MOOVIA_SELFTEST=1` no executável toca o primeiro vídeo da biblioteca pelo caminho real e
imprime o estado do áudio (trilha, volume, mute) por alguns segundos — útil quando algo
funciona no VLC mas não no app.
