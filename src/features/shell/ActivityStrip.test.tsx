import { afterEach, describe, expect, it } from "bun:test";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { SyncProgress, SyncReport } from "../../lib/api";
import { ActivityStrip } from "./ActivityStrip";

const progress: SyncProgress = {
  store: "steam",
  stage: "Reading library",
  done: 4,
  total: 10,
};

const report: SyncReport = {
  owned: 4,
  wishlist: 0,
  removed: 0,
  unmatched: 0,
  failures: [{ store: "epic", account: "account", reason: "expired credentials" }],
  skipped: [],
  cancelled: false,
  family: null,
};

const noActivityProps = {
  operation: null,
  progress: null,
  error: null,
  report: null,
  providerProblems: [],
  exportedPath: null,
  storeName: (store: string) => store,
  onCancel: () => {},
  unmatched: 0,
  onMatch: () => {},
  onDismiss: () => {},
};

describe("ActivityStrip", () => {
  afterEach(cleanup);

  it("keeps progress, cancellation, errors, and export state visible", () => {
    let cancelled = false;
    render(
      <ActivityStrip
        operation="sync"
        progress={progress}
        error="The application could not complete the pass."
        report={report}
        providerProblems={[
          { store: "epic", enabled: true, last_error: "expired credentials" },
        ]}
        exportedPath="/tmp/game-library.json"
        storeName={(store) => store.toUpperCase()}
        onCancel={() => {
          cancelled = true;
        }}
        unmatched={0}
        onMatch={() => {}}
        onDismiss={() => {}}
      />,
    );

    expect(document.querySelector(".activity-operation")?.textContent).toContain("Reading library");
    expect(screen.getByRole("progressbar")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cancelled).toBe(true);
    expect(screen.getAllByRole("alert")[0]?.textContent).toContain("could not complete");
    expect(screen.getByText("EPIC")).toBeDefined();
    expect(screen.getByText("Written to /tmp/game-library.json")).toBeDefined();
  });

  it("shows a successful report as a structured status", () => {
    render(
      <ActivityStrip
        {...noActivityProps}
        report={{
          owned: 4,
          wishlist: 2,
          removed: 1,
          unmatched: 0,
          failures: [],
          skipped: [],
          cancelled: false,
          family: null,
        }}
      />,
    );
    expect(screen.getByText("Synchronisation complete")).toBeDefined();
    expect(document.querySelector(".activity-strip--success")).not.toBeNull();
  });

  it("says how many new copies wait for the matching and offers to run it", () => {
    let matched = false;
    render(
      <ActivityStrip
        {...noActivityProps}
        report={{
          owned: 4,
          wishlist: 2,
          removed: 0,
          unmatched: 2,
          failures: [],
          skipped: [],
          cancelled: false,
          family: null,
        }}
        unmatched={2}
        onMatch={() => {
          matched = true;
        }}
      />,
    );
    expect(document.querySelector(".activity-unmatched")?.textContent).toContain(
      "2 new copies have no record yet",
    );
    fireEvent.click(screen.getByRole("button", { name: "Match" }));
    expect(matched).toBe(true);
  });

  it("says nothing about the matching when every new copy has a record", () => {
    render(
      <ActivityStrip
        {...noActivityProps}
        report={{ ...report, failures: [] }}
      />,
    );
    expect(document.querySelector(".activity-unmatched")).toBeNull();
    expect(screen.queryByRole("button", { name: "Match" })).toBeNull();
  });

  it("labels warning and stopped-pass reports with text", () => {
    render(
      <ActivityStrip
        {...noActivityProps}
        report={report}
      />,
    );
    expect(screen.getByText("Synchronisation completed with warnings")).toBeDefined();
    expect(document.querySelector(".activity-strip--warning")).not.toBeNull();
    expect(screen.getByRole("alert")).toBeDefined();
  });

  it("keeps a stopped pass visible without treating it as an error", () => {
    render(
      <ActivityStrip
        {...noActivityProps}
        report={{
          owned: 2,
          wishlist: 0,
          removed: 0,
          unmatched: 0,
          failures: [],
          skipped: [],
          cancelled: true,
          family: null,
        }}
        stoppedPass="The work made so far is kept."
      />,
    );
    expect(screen.getByText("Pass stopped")).toBeDefined();
    expect(screen.getByText("The work made so far is kept.")).toBeDefined();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("lets the user dismiss a finished report", () => {
    let dismissed = false;
    render(
      <ActivityStrip
        {...noActivityProps}
        report={{ ...report, failures: [] }}
        onDismiss={() => {
          dismissed = true;
        }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(dismissed).toBe(true);
  });

  it("offers no dismiss while an operation runs, or for a provider problem only", () => {
    render(<ActivityStrip {...noActivityProps} operation="sync" report={report} />);
    expect(screen.queryByRole("button", { name: "Dismiss" })).toBeNull();
    cleanup();

    render(
      <ActivityStrip
        {...noActivityProps}
        providerProblems={[{ store: "epic", enabled: true, last_error: "expired credentials" }]}
      />,
    );
    expect(screen.queryByRole("button", { name: "Dismiss" })).toBeNull();
  });

  it("does not render an empty activity strip", () => {
    render(<ActivityStrip {...noActivityProps} />);
    expect(document.querySelector(".activity-strip")).toBeNull();
  });
});
