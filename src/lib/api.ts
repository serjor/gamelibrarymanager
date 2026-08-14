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
}

export interface LibrarySummary {
  owned: number;
  wishlist: number;
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
};

/** Los errores cruzan el puente como texto plano; aquí se normalizan. */
export function errorMessage(cause: unknown): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return "Ha fallado algo y no ha dicho qué.";
}
