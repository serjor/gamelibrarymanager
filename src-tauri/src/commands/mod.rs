use serde::Serialize;

#[derive(Serialize)]
pub struct AppInfo {
    pub version: &'static str,
    pub stores: Vec<&'static str>,
}

/// Primer comando: existe para probar el puente UI↔Rust de extremo a extremo.
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        stores: vec![domain::StoreId::Steam.as_str()],
    }
}
