//! Connection of a GOG account.
//!
//! All of the flow is here because it is the first one that drives a window of
//! its own: the sign in page **of GOG** is opened, the GOG server is left to
//! redirect, and the `code` is taken from that redirect. The password of the
//! user is typed on the GOG domain and never passes through this process.
//!
//! The login window is not in `capabilities/default.json`, and that is
//! deliberate: a remote page cannot invoke one command of ours.

use std::sync::{Arc, Mutex};

use connectors::GogConnector;
use domain::{AuthContext, ClientCredentials, StoreAccount, StoreAccountId, StoreId};
use storage::repositories::StoreAccountRepository;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use time::OffsetDateTime;

use crate::error::AppError;
use crate::state::{AppState, credential_key};

const LOGIN_WINDOW: &str = "gog-login";

/// Connects a GOG account from end to end.
///
/// The user supplies `client_id` and `client_secret`: GOG does not let you
/// register a client of your own and this program carries none inside. They go
/// to the store of secrets with the tokens, because the refresh needs them.
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

    let code = request_code(&app, &client.client_id).await?;

    let connector = state
        .connectors
        .get(&StoreId::Gog)
        .ok_or_else(|| AppError::Message("there is no GOG connector".to_owned()))?;

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

/// Opens the GOG login and waits for the `code`.
///
/// The channel resolves through two ways — the correct redirect or the close of
/// the window — because if it waited only for the first one, a close of the
/// login would leave the command waiting for ever.
async fn request_code(app: &AppHandle, client_id: &str) -> Result<String, AppError> {
    // A half finished earlier attempt would leave two windows and two listeners.
    if let Some(previous) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = previous.close();
    }

    let url = GogConnector::authorize_url("https://auth.gog.com", client_id);
    let url = url
        .parse()
        .map_err(|_| AppError::Message("the GOG login address is not valid".to_owned()))?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    let sender = Arc::new(Mutex::new(Some(tx)));

    let on_navigation = sender.clone();
    let window = WebviewWindowBuilder::new(app, LOGIN_WINDOW, WebviewUrl::External(url))
        .title("Sign in to GOG")
        .inner_size(520.0, 720.0)
        .on_navigation(
            move |url| match GogConnector::code_from_redirect(url.as_str()) {
                Some(code) => {
                    resolve(&on_navigation, Some(code));
                    // The destination page does not need to load: the only data
                    // that it had was the code in its address.
                    false
                }
                None => true,
            },
        )
        .build()
        .map_err(|e| AppError::Message(format!("could not open the GOG login: {e}")))?;

    let on_close = sender.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            resolve(&on_close, None);
        }
    });

    let received = rx.await.unwrap_or(None);
    let _ = window.close();

    received.ok_or_else(|| AppError::Message("the GOG login was cancelled".to_owned()))
}

/// Resolves the channel one time only. The two ways can occur almost together —
/// a close of the window immediately after a correct login — and the second one
/// can neither overwrite the first one nor panic.
fn resolve(
    sender: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<Option<String>>>>>,
    value: Option<String>,
) {
    let Ok(mut guard) = sender.lock() else {
        return;
    };
    if let Some(tx) = guard.take() {
        let _ = tx.send(value);
    }
}
