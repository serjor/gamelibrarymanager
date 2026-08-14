import { invoke } from "@tauri-apps/api/core";

export type SecretsBackend = "keyring" | "passphrase";

export interface AppInfo {
  version: string;
  secrets_backend: SecretsBackend;
  unlocked: boolean;
}

export interface Account {
  store: string;
  account_ref: string;
  display_name: string | null;
  last_sync_at: number | null;
}

export interface SyncFailure {
  store: string;
  account: string;
  reason: string;
}

export interface SyncReport {
  owned: number;
  wishlist: number;
  removed: number;
  failures: SyncFailure[];
  cancelled: boolean;
}

export interface LibrarySummary {
  owned: number;
  wishlist: number;
  games: number;
  pending_review: number;
}

export interface ScoredCandidate {
  igdb_id: number;
  name: string;
  score: number;
}

export interface ReviewItem {
  store_entry_id: string;
  store: string;
  title: string;
  candidates: ScoredCandidate[];
}

export type PlayStatus = "backlog" | "playing" | "finished" | "abandoned";

export interface LibraryRow {
  game_id: string;
  title: string;
  sort_title: string;
  cover_url: string | null;
  release_year: number | null;
  genres: string[];
  owned_stores: string[];
  wishlist_stores: string[];
  playtime_minutes: number;
  status: PlayStatus | null;
  rating: number | null;
  notes: string | null;
}

export interface SyncProgress {
  store: string;
  stage: string;
  done: number;
  total: number;
}

export interface IdentityReport {
  linked: number;
  review: number;
  unknown: number;
}

/** Única puerta hacia Rust. Nadie más llama a `invoke` directamente. */
export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  unlockSecrets: (passphrase: string) => invoke<void>("unlock_secrets", { passphrase }),
  connectSteam: (apiKey: string, steamId: string) =>
    invoke<string>("connect_steam", { apiKey, steamId }),
  listAccounts: () => invoke<Account[]>("list_accounts"),
  syncNow: () => invoke<SyncReport>("sync_now"),
  librarySummary: () => invoke<LibrarySummary>("library_summary"),
  hasIgdbCredentials: () => invoke<boolean>("has_igdb_credentials"),
  setIgdbCredentials: (clientId: string, clientSecret: string) =>
    invoke<void>("set_igdb_credentials", { clientId, clientSecret }),
  resolveIdentities: () => invoke<IdentityReport>("resolve_identities"),
  reviewQueue: () => invoke<ReviewItem[]>("review_queue"),
  reviewConfirm: (storeEntryId: string, igdbId: number) =>
    invoke<void>("review_confirm", { storeEntryId, igdbId }),
  reviewWithoutMetadata: (storeEntryId: string) =>
    invoke<void>("review_without_metadata", { storeEntryId }),
  library: () => invoke<LibraryRow[]>("library"),
  cancelSync: () => invoke<void>("cancel_sync"),
  setUserState: (
    gameId: string,
    status: PlayStatus | null,
    rating: number | null,
    notes: string | null,
  ) => invoke<void>("set_user_state", { gameId, status, rating, notes }),
};

/** Los errores cruzan el puente como texto plano; aquí se normalizan. */
export function errorMessage(cause: unknown): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return "Ha fallado algo y no ha dicho qué.";
}
