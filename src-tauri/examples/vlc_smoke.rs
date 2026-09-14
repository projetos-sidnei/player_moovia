//! Diagnóstico isolado do libVLC: `cargo run --example vlc_smoke`.
//! Reproduz o mesmo mecanismo do app: delay-load + SetDllDirectoryW (sem env var).

#[link(name = "kernel32")]
extern "system" {
    fn SetDllDirectoryW(path: *const u16) -> i32;
}

fn main() {
    let vlc_dir = std::env::var("VLC_DIR").unwrap_or_else(|_| r"C:\Program Files\VideoLAN\VLC".to_string());
    let wide: Vec<u16> = vlc_dir.encode_utf16().chain(std::iter::once(0)).collect();
    let ok = unsafe { SetDllDirectoryW(wide.as_ptr()) };
    println!("SetDllDirectoryW({vlc_dir}): {}", if ok != 0 { "ok" } else { "FALHOU" });
    match vlc::Instance::new() {
        Some(_) => println!("Instance: OK"),
        None => println!("Instance: FALHOU (libvlc_new retornou null)"),
    }
}
