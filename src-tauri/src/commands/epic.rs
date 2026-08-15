//! Connection of an Epic account.
//!
//! The whole flow lives here because it is the second one that drives a window
//! of its own: the sign in page **of Epic** is opened, Epic is left to redirect,
//! and the code is taken from where Epic puts it. The password of the user is
//! typed on the Epic domain and never passes through this process.
//!
//! Where this differs from GOG, and it is the only real difference: GOG ends
//! with a redirection that carries the `code` in the address, so watching the
//! addresses is enough. Epic answers a JSON document, so the page has to be
//! read. That is one step further into a remote page, and it is fenced in:
//!
//! - The script only runs on the address that mints the code, never on the one
//!   where the password is typed.
//! - The script has no logic. It hands the text of the page over and the code
//!   is looked for in Rust, where there are tests.
//! - The login window is not in `capabilities/default.json`, so the remote page
//!   cannot invoke a single command of ours.

use std::sync::{Arc, Mutex};

use connectors::EpicConnector;
use domain::{AuthContext, ClientCredentials, StoreAccount, StoreAccountId, StoreId};
use storage::repositories::StoreAccountRepository;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use time::OffsetDateTime;

use crate::error::AppError;
use crate::state::{AppState, credential_key};

const LOGIN_WINDOW: &str = "epic-login";

/// What is asked of the page that mints the code: its text, and nothing else.
///
/// The reading is deliberately dumb. Parsing here would mean a piece of logic
/// with no test on the other side of the bridge, and the shape of that answer
/// is exactly what will change the day Epic moves.
const READ_BODY: &str = "document.body ? document.body.innerText : null";

/// Connects an Epic account from end to end.
///
/// `client_id` and `client_secret` are handed over by the user: Epic does not
/// let a third party register a client and this program carries none inside.
/// They end up in the secret store next to the tokens, because the refresh
/// needs them.
#[tauri::command]
pub async fn connect_epic(
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
        .get(&StoreId::Epic)
        .ok_or_else(|| AppError::Message("there is no Epic connector".to_owned()))?;

    let session = connector
        .authenticate(&AuthContext::AuthCode { code, client })
        .await?;

    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Epic,
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

/// How a login window ends.
///
/// Three ways and not two, because the two that fail need different words: one
/// is the user changing their mind, and the other is Epic answering something
/// this program no longer understands.
enum Outcome {
    Code(String),
    Unreadable,
    Closed,
}

/// Opens the Epic sign in page and waits for the code.
async fn request_code(app: &AppHandle, client_id: &str) -> Result<String, AppError> {
    // A half finished earlier attempt would leave two windows and two listeners.
    if let Some(previous) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = previous.close();
    }

    let url = EpicConnector::authorize_url(client_id);
    let url = url
        .parse()
        .map_err(|_| AppError::Message("the Epic login address is not valid".to_owned()))?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Outcome>();
    let sender = Arc::new(Mutex::new(Some(tx)));

    let on_load = sender.clone();
    let window = WebviewWindowBuilder::new(app, LOGIN_WINDOW, WebviewUrl::External(url))
        .title("Sign in to Epic")
        .inner_size(520.0, 720.0)
        .on_page_load(move |webview, payload| {
            // Only the finished load, and only the page that carries the code.
            // Everything the user goes through to sign in is left alone.
            if payload.event() != PageLoadEvent::Finished
                || !EpicConnector::is_authorization_page(payload.url().as_str())
            {
                return;
            }

            let sender = on_load.clone();
            let _ = webview.eval_with_callback(READ_BODY, move |result| {
                let outcome = match code_from_eval(&result) {
                    Some(code) => Outcome::Code(code),
                    None => Outcome::Unreadable,
                };
                resolve(&sender, outcome);
            });
        })
        .build()
        .map_err(|e| AppError::Message(format!("could not open the Epic login: {e}")))?;

    let on_close = sender.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            resolve(&on_close, Outcome::Closed);
        }
    });

    let outcome = rx.await.unwrap_or(Outcome::Closed);
    let _ = window.close();

    match outcome {
        Outcome::Code(code) => Ok(code),
        Outcome::Unreadable => Err(AppError::Message(
            "Epic opened the authorisation page but gave back no code. Try \
             again; if this occurs again, Epic has changed how it authorises \
             and the connector needs an update."
                .to_owned(),
        )),
        Outcome::Closed => Err(AppError::Message("the Epic login was cancelled".to_owned())),
    }
}

/// Turns what the webview hands back into the code.
///
/// Two layers: the webview serialises the value of the script as JSON, and
/// inside travels the text of the page, which is the JSON of Epic. Unwrapping
/// the first one here is what lets the second one be read by the connector,
/// which is where it is tested.
fn code_from_eval(result: &str) -> Option<String> {
    let body: String = serde_json::from_str(result).ok()?;
    EpicConnector::code_from_body(&body)
}

/// Resolves the channel once and only once. The ways out can fire almost
/// together —closing the window right after the code arrives— and the second
/// one can neither overwrite the first nor panic.
fn resolve(sender: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<Outcome>>>>, outcome: Outcome) {
    let Ok(mut guard) = sender.lock() else {
        return;
    };
    if let Some(tx) = guard.take() {
        let _ = tx.send(outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_the_two_layers_of_the_answer() {
        // The webview hands the text of the page over as a JSON string, and
        // inside travels the JSON of Epic with the code.
        let result = r#""{\"authorizationCode\":\"ABC123\",\"sid\":null}""#;
        assert_eq!(code_from_eval(result).as_deref(), Some("ABC123"));
    }

    #[test]
    fn a_page_that_cannot_be_read_is_not_a_code() {
        // Empty is what wry hands back when the script throws, and the login
        // page is what arrives if Epic ever stops redirecting.
        assert_eq!(code_from_eval(""), None);
        assert_eq!(code_from_eval("null"), None);
        assert_eq!(code_from_eval(r#""<html>Sign in</html>""#), None);
    }
}
