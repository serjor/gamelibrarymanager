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
  /** Tiendas que se han saltado por tener el conector desactivado. */
  skipped: string[];
  cancelled: boolean;
}

/**
 * Estado de un conector. Solo llegan los que tienen algo que decir: una tienda
 * sin fila está encendida y sin errores.
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
  /** Identificador de la ficha en IGDB, para poder ir a mirarla. */
  slug: string | null;
}

export interface ReviewItem {
  store_entry_id: string;
  store: string;
  title: string;
  /** Lo que enseña la tienda de esta copia, para comparar contra IGDB. */
  cover_url: string | null;
  store_url: string | null;
  candidates: ScoredCandidate[];
  /** Los dos mejores puntúan igual: casi siempre son la misma ficha repetida. */
  tie: boolean;
}

export type PlayStatus = "backlog" | "playing" | "finished" | "abandoned";

export interface LibraryRow {
  game_id: string;
  title: string;
  sort_title: string;
  cover_url: string | null;
  /** El resumen de IGDB. Falta en las fichas nacidas del título de la tienda. */
  summary: string | null;
  release_year: number | null;
  genres: string[];
  owned_stores: string[];
  wishlist_stores: string[];
  /**
   * La imagen apaisada de la tienda, que no es lo mismo que `cover_url`: IGDB
   * sirve carátulas 3:4 y la tienda sirve cabeceras panorámicas. Las dos van
   * juntas con `store_url`, y las dos salen de la misma copia.
   */
  store_cover_url: string | null;
  store_url: string | null;
  playtime_minutes: number;
  /**
   * Última partida, en segundos desde la época. Solo lo publica Steam: un
   * juego que solo esté en GOG lo tiene a `null` aunque se haya jugado, así
   * que no se puede leer como «nunca jugado».
   */
  last_played_at: number | null;
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
  cancelled: boolean;
}

/** Única puerta hacia Rust. Nadie más llama a `invoke` directamente. */
export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  unlockSecrets: (passphrase: string) => invoke<void>("unlock_secrets", { passphrase }),
  connectSteam: (apiKey: string, steamId: string) =>
    invoke<string>("connect_steam", { apiKey, steamId }),
  /** Abre el login de GOG y no resuelve hasta que el usuario termina o cierra. */
  connectGog: (clientId: string, clientSecret: string) =>
    invoke<string>("connect_gog", { clientId, clientSecret }),
  /** Igual que el de GOG, salvo que el código llega en el cuerpo de la página. */
  connectEpic: (clientId: string, clientSecret: string) =>
    invoke<string>("connect_epic", { clientId, clientSecret }),
  listAccounts: () => invoke<Account[]>("list_accounts"),
  connectorStates: () => invoke<ConnectorState[]>("connector_states"),
  /** Apaga o vuelve a encender una tienda sin tocar las demás. */
  setConnectorEnabled: (store: string, enabled: boolean) =>
    invoke<void>("set_connector_enabled", { store, enabled }),
  syncNow: () => invoke<SyncReport>("sync_now"),
  librarySummary: () => invoke<LibrarySummary>("library_summary"),
  hasIgdbCredentials: () => invoke<boolean>("has_igdb_credentials"),
  setIgdbCredentials: (clientId: string, clientSecret: string) =>
    invoke<void>("set_igdb_credentials", { clientId, clientSecret }),
  resolveIdentities: () => invoke<IdentityReport>("resolve_identities"),
  reviewQueue: () => invoke<ReviewItem[]>("review_queue"),
  reviewConfirm: (storeEntryId: string, igdbId: number) =>
    invoke<void>("review_confirm", { storeEntryId, igdbId }),
  /** Confirma varios a la vez. Cada par lo ha elegido el usuario. */
  reviewConfirmMany: (decisions: [string, number][]) =>
    invoke<number>("review_confirm_many", { decisions }),
  reviewWithoutMetadata: (storeEntryId: string) =>
    invoke<void>("review_without_metadata", { storeEntryId }),
  library: () => invoke<LibraryRow[]>("library"),
  /** Para lo que esté corriendo: sincronizar o emparejar. */
  cancelOperation: () => invoke<void>("cancel_operation"),
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
