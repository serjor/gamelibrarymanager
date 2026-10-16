import type { ConnectorState, FamilyOutcome, SyncProgress, SyncReport } from "../../lib/api";

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
  /**
   * The new copies of the report that still have no record: the matching after
   * the synchronisation did not run to the end. Zero hides the notice.
   */
  unmatched: number;
  /** Runs the matching, for the copies that the synchronisation left with no record. */
  onMatch: () => void;
  /**
   * Clears what the last operation left: its report, its error, a stopped pass
   * and the path of an export. The problems of a provider stay, because they
   * come from the connector and not from the last operation.
   */
  onDismiss: () => void;
}

function operationLabel(operation: string): string {
  switch (operation) {
    case "sync":
      return "Synchronising";
    case "identity":
      return "Matching";
    case "prices":
      return "Updating prices";
    case "luna":
      return "Reading the Luna catalogue";
    case "steam-family":
      return "Reading the Steam family";
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
    familySummary(report.family) +
    "."
  );
}

/**
 * The synchronisation writes copies, and the library shows records: a new copy
 * stays out of the library until the matching gives it one. The matching runs
 * after each synchronisation; when it does not run to the end, this notice
 * says why a wish that the store has is not on screen.
 */
function unmatchedNotice(count: number): string {
  return count === 1
    ? "1 new copy has no record yet, and the library does not show it until you match it."
    : String(count) +
        " new copies have no record yet, and the library does not show them until you match them.";
}

/** The part of the summary that the stage of the family gives, when it ended. */
function familySummary(family: FamilyOutcome | null): string {
  switch (family?.outcome) {
    case "updated":
      return "; your Steam family can lend " + String(family.games) + " games";
    case "not_member":
      return "; Steam says that you are in no family";
    default:
      return "";
  }
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
  unmatched,
  onMatch,
  onDismiss,
}: ActivityStripProps) {
  const failures = report?.failures ?? [];
  const skipped = report?.skipped ?? [];
  // A family that did not update is a warning and not a failure: the copies
  // of every store are written, and the marks of the last read stay.
  const familySkipped =
    report?.family?.outcome === "skipped" ? report.family.reason : null;
  const hasReport = report !== null;
  const hasWarnings =
    failures.length > 0 ||
    skipped.length > 0 ||
    familySkipped !== null ||
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

  // What the last operation left can go; a provider problem stays until the
  // provider answers again, because to hide it would hide a library that does
  // not grow.
  const canDismiss =
    operation === null &&
    (error !== null || hasReport || passStopped !== null || exportedPath !== null);

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
          {unmatched > 0 && (
            <span className="activity-unmatched">
              {unmatchedNotice(unmatched)}{" "}
              <button type="button" className="link" onClick={onMatch}>
                Match
              </button>
            </span>
          )}
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

      {familySkipped !== null && (
        <ul className="activity-skipped activity-warning-list" aria-label="Steam Family">
          <li>
            Steam Family was not updated: {familySkipped}. Its marks are the marks of the last
            read.
          </li>
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

      {canDismiss && (
        <button type="button" className="link activity-dismiss" onClick={onDismiss}>
          Dismiss
        </button>
      )}
    </section>
  );
}
