fn main() {
    // libVLC: import lib gerada em vlc-lib/ (ver contexto/decisoes.md) + delay-load.
    // A libvlc.dll precisa ser carregada DO diretório do VLC (plugins são descobertos
    // relativo à libvlccore.dll); o delay-load permite chamar SetDllDirectoryW antes
    // do primeiro uso em vez de resolver a DLL no start do processo.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-search=native={manifest}\\vlc-lib");
    println!("cargo:rustc-link-arg=/DELAYLOAD:libvlc.dll");
    println!("cargo:rustc-link-arg=delayimp.lib");
    tauri_build::build()
}
