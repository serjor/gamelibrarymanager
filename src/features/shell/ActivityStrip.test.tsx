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
  failures: [{ store: "epic", account: "account", reason: "expired credentials" }],
  skipped: [],
  cancelled: false,
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
          failures: [],
          skipped: [],
          cancelled: false,
        }}
      />,
    );
    expect(screen.getByText("Synchronisation complete")).toBeDefined();
    expect(document.querySelector(".activity-strip--success")).not.toBeNull();
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
          failures: [],
          skipped: [],
          cancelled: true,
        }}
        stoppedPass="The work made so far is kept."
      />,
    );
    expect(screen.getByText("Pass stopped")).toBeDefined();
    expect(screen.getByText("The work made so far is kept.")).toBeDefined();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("does not render an empty activity strip", () => {
    render(<ActivityStrip {...noActivityProps} />);
    expect(document.querySelector(".activity-strip")).toBeNull();
  });
});
