import { describe, expect, it, mock, beforeEach } from "bun:test";
import { render, screen } from "@testing-library/react";
import type { Account, AppInfo, LibrarySummary } from "./lib/api";

const state = {
  info: {
    version: "0.1.0",
    secrets_backend: "keyring",
    unlocked: true,
  } as AppInfo,
  accounts: [] as Account[],
  summary: { owned: 0, wishlist: 0 } as LibrarySummary,
};

mock.module("./lib/api", () => ({
  api: {
    appInfo: () => Promise.resolve(state.info),
    listAccounts: () => Promise.resolve(state.accounts),
    librarySummary: () => Promise.resolve(state.summary),
    syncNow: () => Promise.resolve({ owned: 0, wishlist: 0, removed: 0, failures: [] }),
    unlockSecrets: () => Promise.resolve(),
    connectSteam: () => Promise.resolve("id"),
  },
  errorMessage: (cause: unknown) => String(cause),
}));

const { App } = await import("./App");

describe("App", () => {
  beforeEach(() => {
    state.info = { version: "0.1.0", secrets_backend: "keyring", unlocked: true };
    state.accounts = [];
    state.summary = { owned: 0, wishlist: 0 };
  });

  it("sin cuentas conectadas lleva al asistente de Steam", async () => {
    render(<App />);
    expect(await screen.findByText("Conectar Steam")).toBeDefined();
  });

  it("sin llavero en el sistema pide la contraseña antes que nada", async () => {
    state.info = { version: "0.1.0", secrets_backend: "passphrase", unlocked: false };
    render(<App />);
    expect(await screen.findByText("Contraseña del almacén")).toBeDefined();
  });

  it("con una cuenta conectada muestra el recuento de la biblioteca", async () => {
    state.accounts = [
      { store: "steam", account_ref: "7656119", display_name: "serjor", last_sync_at: null },
    ];
    state.summary = { owned: 412, wishlist: 37 };
    render(<App />);
    expect(await screen.findByText(/412 en la biblioteca/)).toBeDefined();
    expect(screen.getByText(/sin sincronizar/)).toBeDefined();
  });
});
