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
  stoppedPass?: string | null;
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

function reportSummary(report: SyncReport): string {
  return (
    "Synchronised " +
    String(report.owned) +
    " owned copies and " +
    String(report.wishlist) +
    " wished-for copies" +
    (report.removed > 0
      ? "; removed " + String(report.removed) + " copies"
      : "") +
    "."
  );
}
export function ActivityStrip({
  operation,
  progress,
  error,
  report,
  stoppedPass,
  providerProblems,
  exportedPath,
  storeName,
  onCancel,
}: ActivityStripProps) {
  const failures = report?.failures ?? [];
  const skipped = report?.skipped ?? [];
  const hasReport = report !== null;
  const hasWarnings =
    failures.length > 0 ||
    skipped.length > 0 ||
    providerProblems.length > 0;
  const passStopped =
    stoppedPass ??
    (report?.cancelled === true
      ? 'The pass stopped. Work made to that point is kept; run it again to continue.'
      : null);
  const hasActivity =
    operation !== null ||
    error !== null ||
    hasReport ||
    passStopped !== null ||
    hasWarnings ||
    exportedPath !== null;

  if (!hasActivity) return null;

  const activityState =
    operation !== null
      ? "progress"
      : error !== null
        ? "error"
        : passStopped !== null
          ? "stopped"
          : hasWarnings
            ? "warning"
            : "success";
  const canCancel = operation === "sync" || operation === "identity" || operation === "prices";

  return (
    <section
      className={`activity-strip activity-strip--${activityState}`}
      aria-label="Activity"
      aria-busy={operation !== null}
    >
      {operation !== null && (
        <div className="activity-operation" role="status" aria-live="polite">
          <strong>{operationLabel(operation)}</strong>
          <span className="activity-state-label">In progress</span>
          {progress !== null && progress.total > 0 ? (
            <span className="activity-progress">
              <progress aria-label="Operation progress" value={progress.done} max={progress.total} />
              <span>
                {progress.stage} · {progress.done} of {progress.total} (
                {Math.floor((progress.done / progress.total) * 100)}%)
              </span>
            </span>
          ) : (
            <span className="hint">Working…</span>
          )}
          {canCancel && (
            <button type="button" className="link activity-cancel" onClick={onCancel}>
              Cancel
            </button>
          )}
        </div>
      )}

      {passStopped !== null && operation === null && (
        <div
          className="activity-status activity-status--stopped"
          role="status"
          aria-live="polite"
        >
          <strong>Pass stopped</strong>
          <span>{passStopped}</span>
        </div>
      )}
      {report !== null && !report.cancelled && operation === null && (
        <div
          className={"activity-status activity-status--" + (hasWarnings ? "warning" : "success")}
          role="status"
          aria-live="polite"
        >
          <strong>
            {hasWarnings
              ? "Synchronisation completed with warnings"
              : "Synchronisation complete"}
          </strong>
          <span>{reportSummary(report)}</span>
        </div>
      )}


      {error !== null && (
        <p className="activity-error" role="alert">
          <strong>Error</strong>
          <span>{error}</span>
        </p>
      )}

      {failures.length > 0 && (
        <ul className="activity-errors activity-warning-list" role="alert" aria-label="Synchronisation warnings">
          {failures.map((failure) => (
            <li key={`${failure.store}:${failure.account}`}>
              {failure.store}: {failure.reason}
            </li>
          ))}
        </ul>
      )}

      {skipped.length > 0 && (
        <ul
          className="activity-skipped activity-warning-list"
          aria-label="Skipped stores"
        >
          {skipped.map((store) => (
            <li key={store}>{storeName(store)} was skipped because its connector is switched off.</li>
          ))}
        </ul>
      )}

      {providerProblems.length > 0 && (
        <ul className="activity-problems activity-warning-list" aria-label="Provider problems" role="alert">
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

      {exportedPath !== null && (
        <div className="activity-status activity-status--success" role="status">
          <strong>Export complete</strong>
          <span className="export-path">Written to {exportedPath}</span>
        </div>
      )}
    </section>
  );
}
