//! Janela filha nativa que hospeda o vídeo do libVLC.
//!
//! O WebView2 do Windows NÃO compõe com janelas nativas atrás dele: uma janela
//! de vídeo abaixo da webview é recortada e nunca desenha, e os pixels
//! "transparentes" da webview revelam o desktop, não o vídeo. Por isso a janela
//! do vídeo fica ACIMA da webview (cobrindo a UI da biblioteca enquanto toca) e
//! os controles do player vivem numa janela sobreposta separada — janelas de
//! topo são compostas entre si pelo DWM (ver commands/player.rs).

use std::ffi::c_void;
use std::ptr::null_mut;

type HWND = *mut c_void;

const WS_CHILD: u32 = 0x4000_0000;
const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
const SS_BLACKRECT: u32 = 0x0000_0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;
const HWND_TOP: isize = 0;
const SW_HIDE: i32 = 0;
const SW_SHOWNOACTIVATE: i32 = 4;

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: HWND,
        menu: HWND,
        instance: HWND,
        param: *mut c_void,
    ) -> HWND;
    fn GetClientRect(hwnd: HWND, rect: *mut Rect) -> i32;
    fn SetWindowPos(hwnd: HWND, insert_after: HWND, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    fn ShowWindow(hwnd: HWND, cmd: i32) -> i32;
}

/// HWND da janela do vídeo, injetado como Tauri `State`.
pub struct VideoHost(pub isize);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Cria a janela do vídeo cobrindo o client area do pai. Nasce oculta: só
/// aparece durante a reprodução, senão cobriria a UI da biblioteca.
pub fn create_video_host(parent: isize) -> isize {
    unsafe {
        let mut rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(parent as HWND, &mut rect);
        let class = wide("STATIC");
        let name = wide("");
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            name.as_ptr(),
            WS_CHILD | WS_CLIPSIBLINGS | SS_BLACKRECT,
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            parent as HWND,
            null_mut(),
            null_mut(),
            null_mut(),
        );
        hwnd as isize
    }
}

/// Mostra (no topo do z-order, acima da webview) ou esconde a janela do vídeo.
pub fn set_video_host_visible(host: isize, visible: bool) {
    unsafe {
        if visible {
            SetWindowPos(
                host as HWND,
                HWND_TOP as HWND,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
            ShowWindow(host as HWND, SW_SHOWNOACTIVATE);
        } else {
            ShowWindow(host as HWND, SW_HIDE);
        }
    }
}

/// Acompanha o resize da janela principal (WindowEvent::Resized).
pub fn resize_video_host(host: isize, width: i32, height: i32) {
    unsafe {
        SetWindowPos(host as HWND, null_mut(), 0, 0, width, height, SWP_NOACTIVATE);
    }
}
