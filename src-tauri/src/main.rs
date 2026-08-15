// Windows: no console in the release build.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Turns off the DMA-BUF renderer of WebKitGTK before anything starts.
///
/// With the proprietary NVIDIA driver on Wayland, WebKitGTK 2.4x and 2.5x
/// negotiates buffers that the driver does not give, the compositor closes the
/// connection, and the application stops during the initialisation of GDK before
/// it shows anything:
///
/// ```text
/// Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display.
/// ```
///
/// The variable must be set **before** GTK initialises, and thus it lives here
/// and not in the development script: this way it applies both to `tauri dev`
/// and to the binary that you distribute, which is where it is really important.
/// A user does not debug a window that does not open, they remove the
/// application.
///
/// The value that comes from the environment stays: a user who wants the
/// renderer can ask for it with `WEBKIT_DISABLE_DMABUF_RENDERER=0`.
///
/// Examined on 2026-08-15 in KDE on Wayland with `webkit2gtk-4.1` 2.52.5 and an
/// NVIDIA RTX 5070 Ti. Without the variable, the binary stops with code 1 and
/// that message; with the variable, the window opens and the webview loads.
#[cfg(target_os = "linux")]
fn avoid_webkit_dmabuf() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: this runs before any thread is created. `set_var` is unsafe
        // only if a different thread reads the environment at the same time, and
        // there is no such thread here.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    avoid_webkit_dmabuf();

    gamelibrarymanager_lib::run()
}
