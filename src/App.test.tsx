import { describe, expect, it, mock, beforeEach } from "bun:test";
import { render, screen } from "@testing-library/react";
import type { Account, AppInfo, LibraryRow, LibrarySummary, ReviewItem } from "./lib/api";

const state = {
  info: { version: "0.1.0", secrets_backend: "keyring", unlocked: true } as AppInfo,
  accounts: [] as Account[],
  hasIgdb: true,
  summary: { owned: 0, wishlist: 0, games: 0, pending_review: 0 } as LibrarySummary,
  queue: [] as ReviewItem[],
  rows: [] as LibraryRow[],
};

// El bus de eventos de Tauri no existe fuera de la ventana de la aplicación.
mock.module("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

mock.module("./lib/api", () => ({
  api: {
    appInfo: () => Promise.resolve(state.info),
    listAccounts: () => Promise.resolve(state.accounts),
    hasIgdbCredentials: () => Promise.resolve(state.hasIgdb),
    librarySummary: () => Promise.resolve(state.summary),
    reviewQueue: () => Promise.resolve(state.queue),
    syncNow: () => Promise.resolve({ owned: 0, wishlist: 0, removed: 0, failures: [] }),
    resolveIdentities: () => Promise.resolve({ linked: 0, review: 0, unknown: 0 }),
    unlockSecrets: () => Promise.resolve(),
    connectSteam: () => Promise.resolve("id"),
    setIgdbCredentials: () => Promise.resolve(),
    reviewConfirm: () => Promise.resolve(),
    reviewWithoutMetadata: () => Promise.resolve(),
    library: () => Promise.resolve(state.rows),
    cancelSync: () => Promise.resolve(),
    setUserState: () => Promise.resolve(),
  },
  errorMessage: (cause: unknown) => String(cause),
}));

const { App } = await import("./App");

const cuentaSteam: Account = {
  store: "steam",
  account_ref: "7656119",
  display_name: "serjor",
  last_sync_at: null,
};

describe("App", () => {
  beforeEach(() => {
    state.info = { version: "0.1.0", secrets_backend: "keyring", unlocked: true };
    state.accounts = [];
    state.hasIgdb = true;
    state.summary = { owned: 0, wishlist: 0, games: 0, pending_review: 0 };
    state.queue = [];
    state.rows = [];
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

  it("con cuenta pero sin IGDB pide las credenciales de metadatos", async () => {
    state.accounts = [cuentaSteam];
    state.hasIgdb = false;
    render(<App />);
    expect(await screen.findByText("Metadatos: IGDB")).toBeDefined();
  });

  it("muestra el recuento de fichas, copias y pendientes", async () => {
    state.accounts = [cuentaSteam];
    state.summary = { owned: 412, wishlist: 37, games: 400, pending_review: 12 };
    render(<App />);
    expect(await screen.findByText(/400 fichas/)).toBeDefined();
    expect(screen.getByText(/12 por revisar/)).toBeDefined();
  });

  it("la biblioteca pinta las fichas con sus tiendas", async () => {
    state.accounts = [cuentaSteam];
    state.rows = [
      {
        game_id: "22222222-2222-7222-8222-222222222222",
        title: "Disco Elysium",
        sort_title: "disco elysium",
        cover_url: null,
        release_year: 2019,
        genres: ["RPG"],
        owned_stores: ["steam", "gog"],
        wishlist_stores: [],
        playtime_minutes: 1240,
        status: null,
        rating: null,
        notes: null,
      },
    ];
    render(<App />);
    // La tarjeta es un botón: su nombre accesible es lo que oye quien no ve la
    // portada, y lleva el título y las tiendas.
    const tarjeta = await screen.findByRole("button", { name: /Disco Elysium/ });
    expect(tarjeta.textContent).toContain("steam · gog");
  });

  it("la cola de revisión ofrece los candidatos y la salida sin ficha", async () => {
    state.accounts = [cuentaSteam];
    state.queue = [
      {
        store_entry_id: "11111111-1111-7111-8111-111111111111",
        store: "gog",
        title: "Disco Elysium - The Final Cut",
        candidates: [
          { igdb_id: 132727, name: "Disco Elysium: The Final Cut", score: 0.97 },
          { igdb_id: 115653, name: "Disco Elysium", score: 0.93 },
        ],
      },
    ];
    render(<App />);
    (await screen.findByRole("button", { name: /Por revisar \(1\)/ })).click();
    expect(await screen.findByText(/Disco Elysium: The Final Cut/)).toBeDefined();
    expect(screen.getByText(/crear ficha con el título de la tienda/)).toBeDefined();
  });
});
