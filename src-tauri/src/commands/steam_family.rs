//! The Steam family: the window that reads the token of the store, and the
//! commands of Utilities.
//!
//! The service of the family does not accept the API key, thus this is the
//! one part of Steam that needs a session signed in to the store. The session
//! is the session of the store in the webview of the application, as in a
//! browser: the user signs in on the page of Steam, and the password never
//! passes through this process. What the program takes is the token that the
//! store gives to its own pages, and it takes it from one page only:
//!
//! - The window opens hidden on the page of the token. With a session, the
//!   script reads the page and the window closes; the user sees nothing.
//! - With no session, the window goes to the sign in page of Steam and shows
//!   itself. After the sign in, Steam sends it back to the page of the token.
//! - The script only runs on the page of the token, never on the page where
//!   the password is typed, and it has no logic: the text of the page is read
//!   in Rust, where there are tests.
//! - The window is not in `capabilities/default.json`, so the remote page
//!   cannot invoke a single command of ours.
//!
//! The token is not kept. The session is: it lives in the cookies of the
//! webview, and switching the family off removes the cookies of Steam.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use connectors::SteamConnector;
use connectors::steam::family::{LOGIN_PAGE, TOKEN_PAGE};
use connectors::steam::{StoreToken, TokenPage};
use serde::Serialize;
use storage::repositories::{SteamFamilyRepository, SteamFamilySettings};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use super::WindowProgress;
use crate::error::AppError;
use crate::family::{self, FamilyOutcome, FamilyTokenSource};
use crate::state::AppState;

const TOKEN_WINDOW: &str = "steam-family";

/// What is asked of the page of the token: its text, and nothing else.
const READ_BODY: &str = "document.body ? document.body.innerText : null";

/// How long the hidden window can take. A store that does not answer must not
/// hold the synchronisation for ever; once the window shows the sign in, the
/// user decides, and the window waits for them.
const HIDDEN_LIMIT: Duration = Duration::from_secs(30);

/// How often the wait looks at the cancel flag and at the limit.
const TICK: Duration = Duration::from_millis(250);

/// The sites whose cookies are the session of Steam. `cookies_for_url` also
/// gives the cookies of the parent domain, `.steampowered.com`.
const STEAM_SITES: [&str; 5] = [
    "https://store.steampowered.com/",
    "https://login.steampowered.com/",
    "https://checkout.steampowered.com/",
    "https://help.steampowered.com/",
    "https://steamcommunity.com/",
];

/// The token from the store in a window of the application.
pub struct WebviewTokens<'a> {
    pub app: AppHandle,
    pub state: &'a AppState,
}

#[async_trait]
impl FamilyTokenSource for WebviewTokens<'_> {
    async fn token(&self) -> Result<Option<StoreToken>, AppError> {
        request_token(&self.app, || self.state.operation_cancelled()).await
    }
}

/// What the card of the family in Utilities shows.
#[derive(Serialize)]
pub struct SteamFamilyView {
    /// `None` when the family is switched off.
    pub settings: Option<SteamFamilySettings>,
}

#[tauri::command]
pub async fn steam_family_settings(
    state: State<'_, AppState>,
) -> Result<SteamFamilyView, AppError> {
    Ok(SteamFamilyView {
        settings: SteamFamilyRepository(&state.db).settings().await?,
    })
}

/// Switches the family on: the sign in, if the store has no session, and the
/// first list.
#[tauri::command]
pub async fn enable_steam_family(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<FamilyOutcome, AppError> {
    let _guard = state.try_begin().ok_or(AppError::Busy)?;
    let secrets = state.secrets().await?;
    let tokens = WebviewTokens {
        app: app.clone(),
        state: &state,
    };
    let progress = WindowProgress { app, state: &state };
    family::enable(
        &state.db,
        secrets.as_ref(),
        &state.steam,
        &tokens,
        &progress,
    )
    .await
}

/// Switches the family off: the list goes, and the session of Steam leaves
/// the webview.
#[tauri::command]
pub async fn disable_steam_family(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let _guard = state.try_begin().ok_or(AppError::Busy)?;
    family::disable(&state.db).await?;
    forget_steam_session(&app)
}

/// Removes the cookies of Steam from the webview, and only those: the
/// sessions of GOG and Epic stay.
///
/// The cookies are read and deleted through the main window because all of
/// the windows of the application share one store of cookies.
pub fn forget_steam_session(app: &AppHandle) -> Result<(), AppError> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    for site in STEAM_SITES {
        let url = site
            .parse()
            .map_err(|_| AppError::Message(format!("{site} is not a valid address")))?;
        let cookies = window
            .cookies_for_url(url)
            .map_err(|e| AppError::Message(format!("could not read the cookies of Steam: {e}")))?;
        for cookie in cookies {
            window.delete_cookie(cookie).map_err(|e| {
                AppError::Message(format!("could not remove the session of Steam: {e}"))
            })?;
        }
    }
    Ok(())
}

/// How the window ends.
enum Outcome {
    Token(StoreToken),
    /// The page of the token said something that this program does not
    /// understand, or it said "no session" after the sign in.
    Unreadable,
    /// The user closed the window.
    Closed,
    /// The hidden window did not get to the page of the token in time.
    TimedOut,
    /// The synchronisation was cancelled while the window waited.
    Cancelled,
}

/// Opens the page of the token and waits for the token.
async fn request_token(
    app: &AppHandle,
    cancelled: impl Fn() -> bool,
) -> Result<Option<StoreToken>, AppError> {
    // A half finished earlier attempt would leave two windows and two
    // listeners.
    if let Some(previous) = app.get_webview_window(TOKEN_WINDOW) {
        let _ = previous.close();
    }

    let (tx, mut rx) = tokio::sync::oneshot::channel::<Outcome>();
    let sender = Arc::new(Mutex::new(Some(tx)));
    let shown = Arc::new(AtomicBool::new(false));

    let on_load = sender.clone();
    let shown_on_load = shown.clone();
    let window = WebviewWindowBuilder::new(
        app,
        TOKEN_WINDOW,
        WebviewUrl::External(address(TOKEN_PAGE)?),
    )
    .title("Sign in to Steam")
    .inner_size(520.0, 760.0)
    .visible(false)
    .on_page_load(move |webview, payload| {
        if payload.event() != PageLoadEvent::Finished {
            return;
        }
        let url = payload.url().as_str();

        if SteamConnector::is_token_page(url) {
            let sender = on_load.clone();
            let shown = shown_on_load.clone();
            let window = webview.clone();
            let _ = webview.eval_with_callback(READ_BODY, move |result| {
                match page_from_eval(&result) {
                    Some(TokenPage::Token(token)) => resolve(&sender, Outcome::Token(token)),
                    // The first time with no session: the user signs in.
                    Some(TokenPage::SignedOut) if !shown.swap(true, Ordering::SeqCst) => {
                        let sign_in = address(LOGIN_PAGE).is_ok_and(|login| {
                            window.navigate(login).is_ok() && window.show().is_ok()
                        });
                        if !sign_in {
                            resolve(&sender, Outcome::Unreadable);
                        }
                        let _ = window.set_focus();
                    }
                    // After the sign in, the page still has no session.
                    Some(TokenPage::SignedOut) | None => resolve(&sender, Outcome::Unreadable),
                }
            });
        } else if shown_on_load.load(Ordering::SeqCst)
            && SteamConnector::is_store_page_after_login(url)
        {
            // The sign in ended on another page of the store and not on
            // the page of the token: the window goes there itself.
            if let Ok(token_page) = address(TOKEN_PAGE) {
                let _ = webview.navigate(token_page);
            }
        }
    })
    .build()
    .map_err(|e| AppError::Message(format!("could not open the Steam store: {e}")))?;

    let on_close = sender.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            resolve(&on_close, Outcome::Closed);
        }
    });

    let started = Instant::now();
    let outcome = loop {
        match tokio::time::timeout(TICK, &mut rx).await {
            Ok(outcome) => break outcome.unwrap_or(Outcome::Closed),
            Err(_) if cancelled() => break Outcome::Cancelled,
            Err(_) if !shown.load(Ordering::SeqCst) && started.elapsed() > HIDDEN_LIMIT => {
                break Outcome::TimedOut;
            }
            Err(_) => {}
        }
    };
    let _ = window.close();
    match outcome {
        Outcome::Token(token) => Ok(Some(token)),
        Outcome::Closed => Ok(None),
        Outcome::Unreadable => Err(AppError::Message(
            "Steam opened the page of the store token but gave back no token. Try again; if \
             this occurs again, Steam has changed the page and the family needs an update."
                .to_owned(),
        )),
        Outcome::TimedOut => Err(AppError::Message(
            "the Steam store did not answer in 30 seconds: the family was not updated".to_owned(),
        )),
        Outcome::Cancelled => Err(AppError::Message(
            "the synchronisation was cancelled before the family".to_owned(),
        )),
    }
}

fn address(url: &str) -> Result<tauri::Url, AppError> {
    url.parse()
        .map_err(|_| AppError::Message(format!("{url} is not a valid address")))
}

/// Turns what the webview hands back into what the page says.
///
/// Two layers: the webview serialises the value of the script as JSON, and
/// inside travels the text of the page, which is the JSON of the store.
fn page_from_eval(result: &str) -> Option<TokenPage> {
    let body: String = serde_json::from_str(result).ok()?;
    SteamConnector::token_from_page(&body)
}

/// Resolves the channel once and only once. The ways out can fire almost
/// together —closing the window right after the token arrives— and the second
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
    fn unwraps_the_two_layers_of_the_page() {
        let result = r#""{\"success\":1,\"data\":[]}""#;
        assert_eq!(page_from_eval(result), Some(TokenPage::SignedOut));
    }

    #[test]
    fn a_page_that_cannot_be_read_is_not_a_page_with_no_session() {
        // Empty is what wry hands back when the script throws, and `null` is
        // what the script gives with no body.
        assert_eq!(page_from_eval(""), None);
        assert_eq!(page_from_eval("null"), None);
    }

    #[test]
    fn the_token_page_of_the_window_is_the_page_where_the_script_runs() {
        assert!(SteamConnector::is_token_page(TOKEN_PAGE));
        assert!(!SteamConnector::is_token_page(LOGIN_PAGE));
    }
}
