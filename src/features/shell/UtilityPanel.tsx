import { useEffect, useRef } from "react";
import type {
  Account,
  ConnectorState,
  ExportFormat,
  LibrarySummary,
} from "../../lib/api";
import { THEME_PREFERENCES, type ThemePreference } from "./theme";

export type SetupTarget = "steam" | "gog" | "epic" | "igdb" | "itad";

export interface StoreOption {
  id: SetupTarget;
  name: string;
}

export interface UtilityPanelProps {
  open: boolean;
  onClose: () => void;
  accounts: Account[];
  missingStores: StoreOption[];
  connectors: ConnectorState[];
  hasIgdb: boolean;
  hasItad: boolean;
  busy: string | null;
  theme: ThemePreference;
  onThemeChange: (preference: ThemePreference) => void;
  summary: LibrarySummary | null;
  storeName: (store: string) => string;
  onSetup: (target: SetupTarget) => void;
  onSync: () => void;
  onMatch: () => void;
  onExport: (format: ExportFormat) => void;
  onDisconnect: (account: Account) => void;
  onToggleConnector: (connector: ConnectorState) => void;
}

function syncLabel(account: Account): string {
  return account.last_sync_at === null
    ? "Not synchronised yet"
    : `Last synchronised ${new Date(account.last_sync_at * 1000).toLocaleString()}`;
}

/**
 * Maintenance actions that do not define the product's first visual hierarchy.
 *
 * The native dialog provides modal focus management, Escape handling, and focus
 * return to the Utilities trigger. The panel only presents state and calls the
 * callbacks that App owns.
 */
export function UtilityPanel({
  open,
  onClose,
  accounts,
  missingStores,
  connectors,
  hasIgdb,
  hasItad,
  busy,
  theme,
  onThemeChange,
  summary,
  storeName,
  onSetup,
  onSync,
  onMatch,
  onExport,
  onDisconnect,
  onToggleConnector,
}: UtilityPanelProps) {
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const element = dialog.current;
    if (element === null) return;

    if (open && !element.open) {
      element.showModal();
    } else if (!open && element.open) {
      element.close();
    }
  }, [open]);

  const close = () => {
    if (dialog.current?.open) dialog.current.close();
    else onClose();
  };

  const setup = (target: SetupTarget) => {
    close();
    onSetup(target);
  };

  return (
    <dialog
      ref={dialog}
      id="utility-dialog"
      className="utility-dialog"
      aria-labelledby="utility-title"
      onClose={onClose}
      onCancel={onClose}
      onClick={(event) => {
        if (event.target === dialog.current) close();
      }}
    >
      <div className="utility-panel">
        <header className="utility-header">
          <div>
            <p className="shell-kicker">Maintenance</p>
            <h2 id="utility-title">Utilities</h2>
          </div>
          <button className="link" onClick={close} aria-label="Close Utilities">
            close
          </button>
        </header>

        <div className="utility-content">
          <section className="utility-section" aria-labelledby="utility-operations">
            <div className="utility-section-heading">
              <div>
                <p className="shell-kicker">Collection upkeep</p>
                <h3 id="utility-operations">Operations</h3>
              </div>
              {summary !== null && (
                <span className="utility-count">{summary.games} records</span>
              )}
            </div>
            <div className="utility-actions">
              <button
                className="primary-action"
                onClick={onSync}
                disabled={busy !== null}
              >
                {busy === "sync" ? "Synchronising…" : "Synchronise"}
              </button>
              <button onClick={onMatch} disabled={busy !== null}>
                {busy === "identity" ? "Matching…" : "Match"}
              </button>
            </div>
            <p className="hint utility-help">
              Synchronise store copies first. Match records when metadata needs an
              update.
            </p>
          </section>

          <section className="utility-section" aria-labelledby="utility-appearance">
            <div className="utility-section-heading">
              <div>
                <p className="shell-kicker">Interface</p>
                <h3 id="utility-appearance">Appearance</h3>
              </div>
            </div>
            <div className="utility-preferences">
              <label htmlFor="utility-theme">Theme</label>
              <select
                id="utility-theme"
                value={theme}
                onChange={(event) => {
                  const next = event.currentTarget.value as ThemePreference;
                  if (THEME_PREFERENCES.includes(next)) onThemeChange(next);
                }}
              >
                {THEME_PREFERENCES.map((preference) => (
                  <option key={preference} value={preference}>
                    {preference[0]!.toUpperCase() + preference.slice(1)}
                  </option>
                ))}
              </select>
            </div>
            <p className="hint utility-help">System follows the operating-system theme.</p>
          </section>

          <section className="utility-section" aria-labelledby="utility-accounts">
            <div className="utility-section-heading">
              <div>
                <p className="shell-kicker">Store access</p>
                <h3 id="utility-accounts">Accounts</h3>
              </div>
              <span className="utility-count">{accounts.length} connected</span>
            </div>

            {accounts.length > 0 ? (
              <ul className="utility-list utility-accounts">
                {accounts.map((account) => (
                  <li key={`${account.store}:${account.account_ref}`}>
                    <div className="utility-item-copy">
                      <strong>{storeName(account.store)}</strong>
                      <span>{account.display_name ?? account.account_ref}</span>
                      <span className="hint">{syncLabel(account)}</span>
                    </div>
                    <button
                      className="link utility-item-action"
                      disabled={busy !== null}
                      onClick={() => onDisconnect(account)}
                    >
                      Disconnect {storeName(account.store)}
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="hint">No store account is connected.</p>
            )}

            {missingStores.length > 0 && (
              <div className="utility-subsection">
                <h4>Add a store</h4>
                <div className="utility-actions utility-actions-wrap">
                  {missingStores.map((store) => (
                    <button
                      key={store.id}
                      className="link"
                      disabled={busy !== null}
                      onClick={() => setup(store.id)}
                    >
                      Connect {store.name}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </section>

          <section className="utility-section" aria-labelledby="utility-providers">
            <div className="utility-section-heading">
              <div>
                <p className="shell-kicker">Optional data</p>
                <h3 id="utility-providers">Providers</h3>
              </div>
            </div>
            <div className="utility-status-list">
              <div className="utility-status-row">
                <div className="utility-item-copy">
                  <strong>IGDB metadata</strong>
                  <span className="hint">
                    {hasIgdb ? "Configured" : "Not configured: titles have no cover art"}
                  </span>
                </div>
                <button className="link" disabled={busy !== null} onClick={() => setup("igdb")}>
                  {hasIgdb ? "Reconfigure IGDB" : "Configure IGDB"}
                </button>
              </div>
              <div className="utility-status-row">
                <div className="utility-item-copy">
                  <strong>IsThereAnyDeal prices</strong>
                  <span className="hint">
                    {hasItad ? "Configured" : "Not configured: wishlist prices are unavailable"}
                  </span>
                </div>
                <button className="link" disabled={busy !== null} onClick={() => setup("itad")}>
                  {hasItad ? "Reconfigure ITAD" : "Configure ITAD"}
                </button>
              </div>
            </div>
          </section>

          <section className="utility-section" aria-labelledby="utility-connectors">
            <div className="utility-section-heading">
              <div>
                <p className="shell-kicker">Store health</p>
                <h3 id="utility-connectors">Connectors</h3>
              </div>
            </div>
            {connectors.length === 0 ? (
              <p className="hint">All store connectors are operating normally.</p>
            ) : (
              <ul className="utility-list utility-connectors">
                {connectors.map((connector) => (
                  <li key={connector.store}>
                    <div className="utility-item-copy">
                      <strong>{storeName(connector.store)}</strong>
                      <span className="hint">
                        {connector.enabled
                          ? connector.last_error ?? "Operating normally."
                          : "Switched off. Its library data stays available."}
                      </span>
                    </div>
                    <button
                      className="link utility-item-action"
                      disabled={busy !== null}
                      onClick={() => onToggleConnector(connector)}
                    >
                      {connector.enabled
                        ? `Switch ${storeName(connector.store)} off`
                        : `Switch ${storeName(connector.store)} on`}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="utility-section" aria-labelledby="utility-export">
            <div className="utility-section-heading">
              <div>
                <p className="shell-kicker">Your copy</p>
                <h3 id="utility-export">Export library</h3>
              </div>
            </div>
            <p className="hint utility-help">
              Save a complete JSON copy or a CSV with your personal status, rating,
              and notes.
            </p>
            <div className="utility-actions">
              <button disabled={busy !== null} onClick={() => onExport("json")}>
                {busy === "export-json" ? "Exporting JSON…" : "Export JSON"}
              </button>
              <button disabled={busy !== null} onClick={() => onExport("csv")}>
                {busy === "export-csv" ? "Exporting CSV…" : "Export CSV"}
              </button>
            </div>
          </section>
        </div>
      </div>
    </dialog>
  );
}
