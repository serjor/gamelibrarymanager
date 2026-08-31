import { describe, expect, it } from "bun:test";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Account, ConnectorState, LibrarySummary } from "../../lib/api";
import { AppShell } from "./AppShell";
import type { ThemePreference } from "./theme";

const account: Account = {
  store: "steam",
  account_ref: "7656119",
  display_name: "serjor",
  last_sync_at: null,
};

const summary: LibrarySummary = {
  owned: 1,
  wishlist: 1,
  games: 1,
  pending_review: 1,
};

const connector: ConnectorState = {
  store: "epic",
  enabled: true,
  last_error: "expired credentials",
};

describe("AppShell", () => {
  it("keeps navigation and maintenance actions reachable", async () => {
    const selectedTheme = { value: null as ThemePreference | null };
    render(
      <AppShell
        tab="library"
        onTabChange={() => {}}
        wishlistCount={1}
        reviewCount={1}
        summary={summary}
        activity={<p>Synchronisation is active.</p>}
        utility={{
          accounts: [account],
          missingStores: [
            { id: "gog", name: "GOG" },
            { id: "epic", name: "Epic" },
          ],
          connectors: [connector],
          hasIgdb: false,
          hasItad: false,
          busy: null,
          theme: "system",
          onThemeChange: (preference) => {
            selectedTheme.value = preference;
          },
          summary,
          storeName: (store) => store,
          onSetup: () => {},
          onSync: () => {},
          onMatch: () => {},
          onExport: () => {},
          onDisconnect: () => {},
          onToggleConnector: () => {},
        }}
      >
        <p>Library content</p>
      </AppShell>,
    );

    for (const name of ["Library", "Today", "Wishlist (1)", "Review (1)", "Utilities"]) {
      expect(await screen.findByRole("button", { name })).toBeDefined();
    }
    expect(screen.getByText("Synchronisation is active.")).toBeDefined();

    fireEvent.click(screen.getByRole("button", { name: "Utilities" }));
    const dialog = await screen.findByRole("dialog");
    for (const name of [
      "Synchronise",
      "Match",
      "Connect GOG",
      "Connect Epic",
      "Disconnect steam",
      "Configure IGDB",
      "Configure ITAD",
      "Switch epic off",
      "Export JSON",
      "Export CSV",
    ]) {
      expect(within(dialog).getByRole("button", { name })).toBeDefined();
    }
    const theme = within(dialog).getByRole("combobox", { name: "Theme" }) as HTMLSelectElement;
    expect(theme.value).toBe("system");
    expect(theme.disabled).toBe(false);
    fireEvent.change(theme, { target: { value: "dark" } });
    expect(selectedTheme.value).toBe("dark");

    fireEvent.click(within(dialog).getByRole("button", { name: "Close Utilities" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("button", { name: "Utilities" }).getAttribute("aria-expanded")).toBe(
      "false",
    );
  });
});
