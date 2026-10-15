//! An opt-in smoke test against the real Steam family of the user.
//!
//! The service of the family does not accept the API key, thus the test needs
//! the token of a session signed in to the store. Sign in to
//! store.steampowered.com in a browser, open
//! `https://store.steampowered.com/pointssummary/ajaxgetasyncconfig`, and give
//! the value of `webapi_token` in `STEAM_WEBAPI_TOKEN`. The names of the
//! members come with the API key that the application keeps in the keyring.
//!
//! It never prints a secret: only the account, the expiry of the token, the
//! counts and a few titles.

use connectors::SteamConnector;
use connectors::steam::TokenPage;
use domain::{AuthContext, FamilyGroup, StoreConnector};
use secrets::{KeyringStore, SecretStore};

const SERVICE: &str = "com.serjor.gamelibrarymanager";

#[tokio::test]
#[ignore = "requires STEAM_WEBAPI_TOKEN, the Steam key in the keyring and network access"]
async fn real_steam_gives_the_family_of_the_user() {
    let raw = std::env::var("STEAM_WEBAPI_TOKEN").expect("STEAM_WEBAPI_TOKEN");
    let page = serde_json::json!({ "success": 1, "data": { "webapi_token": raw.trim() } });
    let Some(TokenPage::Token(token)) = SteamConnector::token_from_page(&page.to_string()) else {
        panic!("STEAM_WEBAPI_TOKEN is not a token");
    };
    let user = token
        .steam_id
        .clone()
        .expect("the token names the account that signed in");
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    println!(
        "account {user}; the token expires in {:?} hours",
        token.expires_at.map(|exp| (exp - now) / 3600)
    );

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("HTTP client");
    let steam = SteamConnector::new(http);

    let group = steam
        .family_group(&token)
        .await
        .expect("GetFamilyGroupForUser");
    let FamilyGroup::Member(id) = group else {
        println!("Steam says that the account is in no family");
        return;
    };
    let mut apps = steam
        .shared_apps(&token, &id, &user)
        .await
        .expect("GetSharedLibraryApps");
    println!("the family can lend {} games", apps.len());

    let credential = KeyringStore::new(SERVICE)
        .get(&format!("steam:{user}"))
        .expect("read the keyring")
        .expect("connect this Steam account in the application first");
    let session = steam
        .authenticate(&AuthContext::Stored { credential })
        .await
        .expect("the stored key");
    let mut ids: Vec<String> = apps
        .iter()
        .flat_map(|app| app.owners.iter().map(|owner| owner.steam_id.clone()))
        .collect();
    ids.sort();
    ids.dedup();
    let names = steam
        .player_names(&session, &ids)
        .await
        .expect("GetPlayerSummaries");
    println!("{} owners, {} with a name", ids.len(), names.len());
    assert_eq!(names.len(), ids.len(), "every owner has a public name");

    apps.sort_by(|a, b| a.title.cmp(&b.title));
    for app in apps.iter().take(5) {
        println!("{} {} ({} owners)", app.app_id, app.title, app.owners.len());
    }
}
