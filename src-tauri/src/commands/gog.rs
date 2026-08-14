//! Conexión de una cuenta de GOG.
//!
//! Todo el flujo está aquí porque es el único que gobierna una ventana: se abre
//! la página de login **de GOG**, se espera a que su servidor redirija, y de esa
//! redirección se saca el `code`. La contraseña del usuario se escribe en el
//! dominio de GOG y no pasa por este proceso en ningún momento.
//!
//! La ventana de login no aparece en `capabilities/default.json`, y eso es
//! deliberado: una página remota no puede invocar ni un solo comando nuestro.

use std::sync::{Arc, Mutex};

use connectors::GogConnector;
use domain::{AuthContext, ClientCredentials, StoreAccount, StoreAccountId, StoreId};
use storage::repositories::StoreAccountRepository;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use time::OffsetDateTime;

use crate::error::AppError;
use crate::state::{AppState, credential_key};

const LOGIN_WINDOW: &str = "gog-login";

/// Conecta una cuenta de GOG de principio a fin.
///
/// `client_id` y `client_secret` los aporta el usuario: GOG no permite registrar
/// un cliente propio y este programa no lleva ninguno dentro. Acaban en el
/// almacén de secretos junto a los tokens, porque el refresco los necesita.
#[tauri::command]
pub async fn connect_gog(
    app: AppHandle,
    state: State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> Result<StoreAccountId, AppError> {
    let client = ClientCredentials {
        client_id: client_id.trim().to_owned(),
        client_secret: client_secret.trim().to_owned(),
    };

    let code = pedir_codigo(&app, &client.client_id).await?;

    let connector = state
        .connectors
        .get(&StoreId::Gog)
        .ok_or_else(|| AppError::Message("sin conector de GOG".to_owned()))?;

    let session = connector
        .authenticate(&AuthContext::AuthCode { code, client })
        .await?;

    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Gog,
        account_ref: session.account_ref.clone(),
        display_name: session.display_name.clone(),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let id = StoreAccountRepository(&state.db).upsert(&account).await?;

    state
        .secrets()
        .await?
        .set(&credential_key(&account), &session.credential)?;

    Ok(id)
}

/// Abre el login de GOG y espera al `code`.
///
/// El canal se resuelve por dos vías —la redirección buena o el cierre de la
/// ventana— porque si solo se esperase la primera, cerrar el login dejaría el
/// comando colgado para siempre.
async fn pedir_codigo(app: &AppHandle, client_id: &str) -> Result<String, AppError> {
    // Una sesión anterior a medias dejaría dos ventanas y dos escuchas.
    if let Some(previa) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = previa.close();
    }

    let url = GogConnector::authorize_url("https://auth.gog.com", client_id);
    let url = url
        .parse()
        .map_err(|_| AppError::Message("la dirección de login de GOG no es válida".to_owned()))?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    let emisor = Arc::new(Mutex::new(Some(tx)));

    let al_navegar = emisor.clone();
    let ventana = WebviewWindowBuilder::new(app, LOGIN_WINDOW, WebviewUrl::External(url))
        .title("Iniciar sesión en GOG")
        .inner_size(520.0, 720.0)
        .on_navigation(
            move |url| match GogConnector::code_from_redirect(url.as_str()) {
                Some(code) => {
                    resolver(&al_navegar, Some(code));
                    // No hace falta cargar la página de destino: lo único que
                    // interesaba de ella era el código que trae en la dirección.
                    false
                }
                None => true,
            },
        )
        .build()
        .map_err(|e| AppError::Message(format!("no se pudo abrir el login de GOG: {e}")))?;

    let al_cerrar = emisor.clone();
    ventana.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            resolver(&al_cerrar, None);
        }
    });

    let recibido = rx.await.unwrap_or(None);
    let _ = ventana.close();

    recibido.ok_or_else(|| AppError::Message("login de GOG cancelado".to_owned()))
}

/// Resuelve el canal una sola vez. Las dos vías pueden dispararse casi a la vez
/// —cerrar la ventana justo después de acertar el login— y la segunda no puede
/// pisar a la primera ni entrar en pánico.
fn resolver(
    emisor: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<Option<String>>>>>,
    valor: Option<String>,
) {
    let Ok(mut guard) = emisor.lock() else {
        return;
    };
    if let Some(tx) = guard.take() {
        let _ = tx.send(valor);
    }
}
