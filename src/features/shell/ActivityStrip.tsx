import type { ConnectorState, SyncProgress, SyncReport } from "../../lib/api";

/**
 * The state that must stay visible while the utility dialog is closed.
 *
 * A long operation can change the collection for several minutes. A provider
 * can fail while the rest of the collection stays usable. Both states need a
 * visible place in the shell, not only a place inside Utilities.
 */
export interface ActivityStripProps {
  operation: string | null;
  progress: SyncProgress | null;
  error: string | null;
  report: SyncReport | null;
  providerProblems: ConnectorState[];
  exportedPath: string | null;
  storeName: (store: string) => string;
  onCancel: () => void;
}

function operationLabel(operation: string): string {
  switch (operation) {
    case "sync":
      return "Synchronising";
    case "identity":
      return "Matching";
    case "prices":
      return "Updating prices";
    case "disconnect":
      return "Disconnecting";
    case "connector":
      return "Updating connector";
    case "export-json":
      return "Exporting JSON";
    case "export-csv":
      return "Exporting CSV";
    default:
      return operation;
  }
}

export function ActivityStrip({
  operation,
  progress,
  error,
  report,
  providerProblems,
  exportedPath,
  storeName,
  onCancel,
}: ActivityStripProps) {
  const failures = report?.failures ?? [];
  const hasActivity =
    operation !== null ||
    error !== null ||
    failures.length > 0 ||
    providerProblems.length > 0 ||
    exportedPath !== null;

  if (!hasActivity) return null;

  const canCancel = operation === "sync" || operation === "identity" || operation === "prices";

  return (
    <section className="activity-strip" aria-label="Activity">
      {operation !== null && (
        <div className="activity-operation" role="status" aria-live="polite">
          <strong>{operationLabel(operation)}</strong>
          {progress !== null && progress.total > 0 ? (
            <span className="activity-progress">
              <progress value={progress.done} max={progress.total} />
              <span>
                {progress.stage} · {progress.done} of {progress.total} (
                {Math.floor((progress.done / progress.total) * 100)}%)
              </span>
            </span>
          ) : (
            <span className="hint">Working…</span>
          )}
          {canCancel && (
            <button className="link activity-cancel" onClick={onCancel}>
              Cancel
            </button>
          )}
        </div>
      )}

      {error !== null && (
        <p className="activity-error" role="alert">
          {error}
        </p>
      )}

      {failures.length > 0 && (
        <ul className="activity-errors" role="alert">
          {failures.map((failure) => (
            <li key={`${failure.store}:${failure.account}`}>
              {failure.store}: {failure.reason}
            </li>
          ))}
        </ul>
      )}

      {providerProblems.length > 0 && (
        <ul className="activity-problems" aria-label="Provider problems">
          {providerProblems.map((connector) => (
            <li key={connector.store}>
              <strong>{storeName(connector.store)}</strong>{" "}
              {connector.enabled
                ? `could not synchronise: ${connector.last_error ?? "unknown error"}`
                : "is switched off: it does not synchronise, and the data that it gave stays in the library."}
            </li>
          ))}
        </ul>
      )}

      {exportedPath !== null && <p className="hint export-path">Written to {exportedPath}</p>}
    </section>
  );
}
