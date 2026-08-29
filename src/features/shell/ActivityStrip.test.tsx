import { describe, expect, it } from "bun:test";
import { fireEvent, render, screen } from "@testing-library/react";
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

describe("ActivityStrip", () => {
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

    expect(screen.getByRole("status").textContent).toContain("Reading library");
    expect(screen.getByRole("progressbar")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cancelled).toBe(true);
    expect(screen.getAllByRole("alert")[0]?.textContent).toContain("could not complete");
    expect(screen.getByText("EPIC")).toBeDefined();
    expect(screen.getByText("Written to /tmp/game-library.json")).toBeDefined();
  });
});
