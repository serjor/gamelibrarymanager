mod commands;

/// Arranca la aplicación. Los comandos solo orquestan casos de uso: la lógica
/// vive en los crates de dominio y adaptadores, nunca aquí.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("error al arrancar la aplicación");
}
