// Windows: sin consola en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Apaga el renderizador DMA-BUF de WebKitGTK antes de que arranque nada.
///
/// Con el driver propietario de NVIDIA bajo Wayland, WebKitGTK 2.4x y 2.5x
/// negocia búferes que el driver no le sirve, el compositor corta la conexión y
/// la aplicación se cierra en la inicialización de GDK sin llegar a pintar:
///
/// ```text
/// Gdk-Message: Error 71 (Error de protocolo) dispatching to Wayland display.
/// ```
///
/// Tiene que estar puesta **antes** de que GTK se inicialice, y por eso vive
/// aquí y no en el script de desarrollo: así vale igual para `tauri dev` y para
/// el binario que se distribuye, que es donde de verdad importa. Una ventana
/// que no abre no se depura, se desinstala.
///
/// Se respeta lo que ya venga del entorno: quien quiera el renderizador puede
/// pedirlo con `WEBKIT_DISABLE_DMABUF_RENDERER=0`.
///
/// Vigencia: comprobado el 2026-08-15 en KDE sobre Wayland con `webkit2gtk-4.1`
/// 2.52.5 y una NVIDIA RTX 5070 Ti. Sin la variable, el binario sale con código
/// 1 y ese mensaje; con ella, la ventana abre y el webview carga.
#[cfg(target_os = "linux")]
fn sortear_dmabuf_de_webkit() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: antes de crear ningún hilo. `set_var` solo es insegura si
        // otro hilo está leyendo el entorno a la vez, y aquí no hay ninguno.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    sortear_dmabuf_de_webkit();

    gamelibrarymanager_lib::run()
}
