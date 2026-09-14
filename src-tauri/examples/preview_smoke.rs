//! Valida a captura de frames fora da reprodução:
//! `cargo run --example preview_smoke -- "<arquivo>" [segundos]`

use player_moovia_lib::vlc_engine::preview::PreviewHandle;

#[link(name = "kernel32")]
extern "system" {
    fn SetDllDirectoryW(path: *const u16) -> i32;
}

fn main() {
    let vlc_dir = std::env::var("VLC_DIR").unwrap_or_else(|_| r"C:\Program Files\VideoLAN\VLC".to_string());
    let wide: Vec<u16> = vlc_dir.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { SetDllDirectoryW(wide.as_ptr()) };

    let path = std::env::args().nth(1).expect("uso: preview_smoke <arquivo> [segundos]");
    let seconds: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(60.0);

    let preview = PreviewHandle::spawn();
    match preview.open(&path) {
        Ok(()) => println!("open: OK"),
        Err(e) => {
            println!("open: FALHOU ({e})");
            return;
        }
    }

    for t in [seconds, seconds + 300.0] {
        let started = std::time::Instant::now();
        match preview.grab(t) {
            Some(uri) => {
                println!("grab {t}s: OK em {:?} — data URI com {} bytes", started.elapsed(), uri.len());
                if let Some(dir) = std::env::args().nth(3) {
                    let b64 = uri.split(',').nth(1).unwrap_or_default();
                    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64).unwrap();
                    let file = format!("{dir}/preview_{t}.png");
                    std::fs::write(&file, bytes).unwrap();
                    println!("  salvo em {file}");
                }
            }
            None => println!("grab {t}s: FALHOU (nenhum frame)"),
        }
    }
}
