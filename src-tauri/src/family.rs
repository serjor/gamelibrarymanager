//! The Steam family use case: to keep the list of the games that the other
//! members can lend.
//!
//! It is a stage of the synchronisation, after the accounts, because the user
//! asked for it: it is Steam and not a third party, and a family that changes
//! must not need a button of its own. But its failure is its own. A store
//! session that expired or a sign in that the user closed skips the family and
//! keeps the list of the last stage; the copies of Steam, GOG and Epic are
//! already written by then and nothing of them changes.
//!
//! It writes in `steam_family` and `steam_family_app` and nowhere else. The
//! match with the records happens when the library is read, in
//! `storage::repositories::library`, thus nothing here touches `game`,
//! `store_entry` or `user_state`.
//!
//! The token of the store comes from a `FamilyTokenSource`. In the application
//! it is a hidden window with the store; in the tests it is a fixed token. The
//! token lives in memory during the stage and is never written.

use std::collections::BTreeSet;

use async_trait::async_trait;
use connectors::SteamConnector;
use connectors::steam::StoreToken;
use domain::{AuthContext, ConnectorError, FamilyGroup, StoreConnector, StoreId};
use secrets::SecretStore;
use serde::Serialize;
use storage::Database;
use storage::repositories::{
    ConnectorStateRepository, SteamFamilyRepository, StoreAccountRepository,
};
use time::OffsetDateTime;

use crate::error::AppError;
use crate::state::credential_key;
use crate::sync::{ProgressSink, SyncProgress};

/// Where the token of the store session comes from.
#[async_trait]
pub trait FamilyTokenSource: Send + Sync {
    /// A token of the session signed in to the store. `None` says that the
    /// user closed the sign in: it is a decision, not a failure.
    async fn token(&self) -> Result<Option<StoreToken>, AppError>;
}

/// A token that was already read. Switching the family on reads it before it
/// keeps anything, and the first stage uses the same token.
pub struct FixedToken(pub StoreToken);

#[async_trait]
impl FamilyTokenSource for FixedToken {
    async fn token(&self) -> Result<Option<StoreToken>, AppError> {
        Ok(Some(self.0.clone()))
    }
}

/// How the stage of the family ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum FamilyOutcome {
    /// The list was replaced. `games` is how many games the family can lend.
    Updated { games: usize },
    /// Steam says that the user is in no family. The list is empty.
    NotMember,
    /// The stage did not end. The list of the last stage stays, and the reason
    /// is kept for Utilities.
    Skipped { reason: String },
}

/// The stage of the synchronisation. `None` when the family is switched off:
/// then nothing is asked, not even for a token.
///
/// A failure of the database goes up, as everywhere. Any other failure is a
/// result of the stage and not an error of the synchronisation.
pub async fn sync_family(
    db: &Database,
    secrets: &dyn SecretStore,
    steam: &SteamConnector,
    tokens: &dyn FamilyTokenSource,
    progress: &dyn ProgressSink,
) -> Result<Option<FamilyOutcome>, AppError> {
    let family = SteamFamilyRepository(db);
    if family.settings().await?.is_none() {
        return Ok(None);
    }

    let outcome = match stage(db, secrets, steam, tokens, progress).await? {
        Ok(outcome) => outcome,
        Err(reason) => {
            family.record_error(&reason).await?;
            FamilyOutcome::Skipped { reason }
        }
    };
    Ok(Some(outcome))
}

/// Switches the family on and reads it at once.
///
/// It reads the family **before** it keeps the switch on, as Luna reads the
/// catalogue before it keeps the country: a sign in that the user closed, or a
/// session that Steam refuses, must not leave the family switched on with
/// nothing to show.
pub async fn enable(
    db: &Database,
    secrets: &dyn SecretStore,
    steam: &SteamConnector,
    tokens: &dyn FamilyTokenSource,
    progress: &dyn ProgressSink,
) -> Result<FamilyOutcome, AppError> {
    if let Err(reason) = steam_account(db).await? {
        return Err(AppError::Message(reason));
    }
    let token = tokens.token().await?.ok_or_else(|| {
        AppError::Message("the Steam sign in was closed: the family stays switched off".to_owned())
    })?;

    let family = SteamFamilyRepository(db);
    family.enable().await?;
    match stage(db, secrets, steam, &FixedToken(token), progress).await? {
        Ok(outcome) => Ok(outcome),
        Err(reason) => {
            family.disable().await?;
            Err(AppError::Message(reason))
        }
    }
}

/// Switches the family off. The marks go with the list; nothing else changes.
/// The session of the store is not here: the command that calls this removes
/// it from the webview.
pub async fn disable(db: &Database) -> Result<(), AppError> {
    Ok(SteamFamilyRepository(db).disable().await?)
}

/// The steps of the stage. The outer `Result` is the database, and the inner
/// one is the reason why the stage did not end.
async fn stage(
    db: &Database,
    secrets: &dyn SecretStore,
    steam: &SteamConnector,
    tokens: &dyn FamilyTokenSource,
    progress: &dyn ProgressSink,
) -> Result<Result<FamilyOutcome, String>, AppError> {
    let account = match steam_account(db).await? {
        Ok(account) => account,
        Err(reason) => return Ok(Err(reason)),
    };

    progress.report(SyncProgress {
        store: StoreId::Steam.as_str().to_owned(),
        stage: "Steam family",
        done: 0,
        total: 1,
    });

    let token = match tokens.token().await {
        Ok(Some(token)) => token,
        Ok(None) => {
            return Ok(Err(
                "the Steam sign in was closed: the family was not updated".to_owned(),
            ));
        }
        Err(error) => return Ok(Err(error.to_string())),
    };

    // The webview can be signed in to another account than the one that is
    // connected. Its family is not the family of the user of this library.
    if let Some(signed_in) = &token.steam_id
        && signed_in != &account.account_ref
    {
        return Ok(Err(format!(
            "the Steam store is signed in with another account ({signed_in}): switch the \
             family off and on, and sign in with the connected account"
        )));
    }

    let group = match steam.family_group(&token).await {
        Ok(group) => group,
        Err(error) => return Ok(Err(reason(error))),
    };

    let apps = match &group {
        FamilyGroup::NotMember => Vec::new(),
        FamilyGroup::Member(id) => {
            let mut apps = match steam.shared_apps(&token, id, &account.account_ref).await {
                Ok(apps) => apps,
                Err(error) => return Ok(Err(reason(error))),
            };
            name_owners(secrets, steam, &account, &mut apps).await;
            apps
        }
    };

    let written = SteamFamilyRepository(db)
        .replace(&group, &apps, OffsetDateTime::now_utc())
        .await?;
    if !written {
        return Ok(Err(
            "the family was switched off during the synchronisation".to_owned(),
        ));
    }

    Ok(Ok(match group {
        FamilyGroup::NotMember => FamilyOutcome::NotMember,
        FamilyGroup::Member(_) => FamilyOutcome::Updated { games: apps.len() },
    }))
}

/// The connected Steam account, or why there is none to use.
async fn steam_account(db: &Database) -> Result<Result<domain::StoreAccount, String>, AppError> {
    let switched_off = ConnectorStateRepository(db)
        .all()
        .await?
        .into_iter()
        .any(|state| state.store == StoreId::Steam && !state.enabled);
    if switched_off {
        return Ok(Err(
            "the Steam connector is switched off: the family was not updated".to_owned(),
        ));
    }
    Ok(StoreAccountRepository(db)
        .active()
        .await?
        .into_iter()
        .find(|account| account.store == StoreId::Steam)
        .ok_or_else(|| "connect a Steam account to read its family".to_owned()))
}

/// Puts the names of the profiles on the owners. It is the best that it can
/// do: with no names, the family can still lend the games, and the mark only
/// cannot say by whom. Thus a failure here is not a failure of the stage.
async fn name_owners(
    secrets: &dyn SecretStore,
    steam: &SteamConnector,
    account: &domain::StoreAccount,
    apps: &mut [domain::FamilyApp],
) {
    let ids: Vec<String> = apps
        .iter()
        .flat_map(|app| app.owners.iter().map(|owner| owner.steam_id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if ids.is_empty() {
        return;
    }
    let Ok(Some(credential)) = secrets.get(&credential_key(account)) else {
        return;
    };
    let Ok(mut session) = steam
        .authenticate(&AuthContext::Stored { credential })
        .await
    else {
        return;
    };
    session.account_ref = account.account_ref.clone();
    let Ok(names) = steam.player_names(&session, &ids).await else {
        return;
    };
    for owner in apps.iter_mut().flat_map(|app| app.owners.iter_mut()) {
        owner.name = names.get(&owner.steam_id).cloned();
    }
}

/// The words for a failure of Steam. "Connect the account again" is what the
/// connector says for a refused key, and here the key is not the problem.
fn reason(error: ConnectorError) -> String {
    match error {
        ConnectorError::Unauthorized => "Steam did not accept the store session: switch the \
             family off and on in Utilities to sign in again"
            .to_owned(),
        other => format!("the Steam family: {other}"),
    }
}
