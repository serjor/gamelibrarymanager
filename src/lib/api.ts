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
  /** The stores left out because their connector is switched off. */
  skipped: string[];
  cancelled: boolean;
}

/**
 * The state of a connector. Only the connectors that have something to say come
 * here: a store with no row is on and has no error.
 */
export interface ConnectorState {
  store: string;
  enabled: boolean;
  last_error: string | null;
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
  release_year: number | null;
  cover_url: string | null;
  /** The identifier of the record in IGDB, so that you can go and look at it. */
  slug: string | null;
}

export interface ReviewItem {
  store_entry_id: string;
  store: string;
  title: string;
  /** What the store shows about this copy, to compare against IGDB. */
  cover_url: string | null;
  store_url: string | null;
  candidates: ScoredCandidate[];
  /** The two best have the same score: almost always the same record repeated. */
  tie: boolean;
}

export type PlayStatus = "backlog" | "playing" | "finished" | "abandoned";

export interface LibraryRow {
  game_id: string;
  title: string;
  sort_title: string;
  cover_url: string | null;
  /** The summary from IGDB. Absent in the records made from the store title. */
  summary: string | null;
  release_year: number | null;
  genres: string[];
  owned_stores: string[];
  wishlist_stores: string[];
  /**
   * The horizontal image of the store, which is not the same as `cover_url`:
   * IGDB gives 3:4 covers and the store gives wide headers. The two go together
   * with `store_url`, and the two come from the same copy.
   */
  store_cover_url: string | null;
  store_url: string | null;
  playtime_minutes: number;
  /**
   * The last time played, in seconds from the epoch. Only Steam publishes it: a
   * game that is only in GOG keeps `null` even if the user has played it, thus
   * you cannot read it as "never played".
   */
  last_played_at: number | null;
  status: PlayStatus | null;
  rating: number | null;
  notes: string | null;
}

/**
 * The price of a wished-for game: the least expensive offer at this moment, and
 * the lowest price that it has had.
 *
 * The quantities are cents. They are formatted when they are shown and not
 * before: a price is a count, and in floating point 19.99 stops being 19.99 as
 * soon as you calculate with it.
 */
export interface PriceRow {
  game_id: string;
  /** The store that sells it at the lowest price, with the name that ITAD gives. */
  shop: string;
  amount: number;
  regular: number;
  /** The discount as a percentage, as ITAD calculates it. */
  cut: number;
  currency: string;
  /** How many stores sell it at this moment. */
  shops: number;
  low_all_time: number | null;
  low_year: number | null;
  /** The name with which ITAD publishes the page of the game. */
  itad_slug: string | null;
  captured_at: number;
}

export interface PriceReport {
  priced: number;
  unknown: number;
  cancelled: boolean;
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
  cancelled: boolean;
  /**
   * The provider stopped the requests and the pass stopped there, with the
   * reason. The matches made to that point are kept: a second click continues
   * from there.
   */
  stopped: string | null;
}

/** The only door to Rust. Nobody else calls `invoke` directly. */
export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  unlockSecrets: (passphrase: string) => invoke<void>("unlock_secrets", { passphrase }),
  connectSteam: (apiKey: string, steamId: string) =>
    invoke<string>("connect_steam", { apiKey, steamId }),
  /** Opens the GOG login and resolves when the user finishes or closes it. */
  connectGog: (clientId: string, clientSecret: string) =>
    invoke<string>("connect_gog", { clientId, clientSecret }),
  /** As the GOG login, but the code comes in the body of the page. */
  connectEpic: (clientId: string, clientSecret: string) =>
    invoke<string>("connect_epic", { clientId, clientSecret }),
  listAccounts: () => invoke<Account[]>("list_accounts"),
  connectorStates: () => invoke<ConnectorState[]>("connector_states"),
  /** Switches a store off, or on again, and does not touch the other stores. */
  setConnectorEnabled: (store: string, enabled: boolean) =>
    invoke<void>("set_connector_enabled", { store, enabled }),
  syncNow: () => invoke<SyncReport>("sync_now"),
  librarySummary: () => invoke<LibrarySummary>("library_summary"),
  hasIgdbCredentials: () => invoke<boolean>("has_igdb_credentials"),
  setIgdbCredentials: (clientId: string, clientSecret: string) =>
    invoke<void>("set_igdb_credentials", { clientId, clientSecret }),
  hasItadCredentials: () => invoke<boolean>("has_itad_credentials"),
  /** The ITAD key and the country: with no country, the prices are those of a
   *  different place. */
  setItadCredentials: (key: string, country: string) =>
    invoke<void>("set_itad_credentials", { key, country }),
  refreshPrices: () => invoke<PriceReport>("refresh_prices"),
  prices: () => invoke<PriceRow[]>("prices"),
  resolveIdentities: () => invoke<IdentityReport>("resolve_identities"),
  reviewQueue: () => invoke<ReviewItem[]>("review_queue"),
  reviewConfirm: (storeEntryId: string, igdbId: number) =>
    invoke<void>("review_confirm", { storeEntryId, igdbId }),
  /** Confirms more than one together. The user selected each pair. */
  reviewConfirmMany: (decisions: [string, number][]) =>
    invoke<number>("review_confirm_many", { decisions }),
  reviewWithoutMetadata: (storeEntryId: string) =>
    invoke<void>("review_without_metadata", { storeEntryId }),
  library: () => invoke<LibraryRow[]>("library"),
  /** Stops the operation in progress: a synchronisation or a match. */
  cancelOperation: () => invoke<void>("cancel_operation"),
  setUserState: (
    gameId: string,
    status: PlayStatus | null,
    rating: number | null,
    notes: string | null,
  ) => invoke<void>("set_user_state", { gameId, status, rating, notes }),
};

/** The errors cross the bridge as plain text; they are normalised here. */
export function errorMessage(cause: unknown): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return "Something failed and did not say what.";
}
