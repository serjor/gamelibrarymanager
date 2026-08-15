import { describe, expect, it, mock, beforeEach } from "bun:test";
import { render, screen } from "@testing-library/react";
import type {
  Account,
  AppInfo,
  ConnectorState,
  LibraryRow,
  LibrarySummary,
  ReviewItem,
} from "./lib/api";

const state = {
  info: { version: "0.1.0", secrets_backend: "keyring", unlocked: true } as AppInfo,
  accounts: [] as Account[],
  connectors: [] as ConnectorState[],
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
    connectorStates: () => Promise.resolve(state.connectors),
    setConnectorEnabled: (store: string, enabled: boolean) => {
      state.connectors = state.connectors.map((connector) =>
        connector.store === store ? { ...connector, enabled } : connector,
      );
      return Promise.resolve();
    },
    hasIgdbCredentials: () => Promise.resolve(state.hasIgdb),
    librarySummary: () => Promise.resolve(state.summary),
    reviewQueue: () => Promise.resolve(state.queue),
    syncNow: () =>
      Promise.resolve({ owned: 0, wishlist: 0, removed: 0, failures: [], skipped: [] }),
    resolveIdentities: () =>
      Promise.resolve({ linked: 0, review: 0, unknown: 0, cancelled: false }),
    unlockSecrets: () => Promise.resolve(),
    connectSteam: () => Promise.resolve("id"),
    connectGog: () => Promise.resolve("id"),
    connectEpic: () => Promise.resolve("id"),
    setIgdbCredentials: () => Promise.resolve(),
    reviewConfirm: () => Promise.resolve(),
    reviewConfirmMany: () => Promise.resolve(0),
    reviewWithoutMetadata: () => Promise.resolve(),
    library: () => Promise.resolve(state.rows),
    cancelOperation: () => Promise.resolve(),
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

const cuentaGog: Account = {
  store: "gog",
  account_ref: "51000000000000000",
  display_name: "serjor",
  last_sync_at: null,
};

const cuentaEpic: Account = {
  store: "epic",
  account_ref: "a1b2c3d4e5f64788b0c1d2e3f4a5b6c7",
  display_name: "serjor",
  last_sync_at: null,
};

describe("App", () => {
  beforeEach(() => {
    state.info = { version: "0.1.0", secrets_backend: "keyring", unlocked: true };
    state.accounts = [];
    state.connectors = [];
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

  it("sin IGDB avisa pero no bloquea la biblioteca", async () => {
    // La ficha nace del emparejamiento, así que sin IGDB sale del título de la
    // tienda. Es un aviso, no un error: cerrar la aplicación entera hasta tener
    // credenciales de Twitch es demasiado duro en el primer arranque.
    state.accounts = [cuentaSteam];
    state.hasIgdb = false;
    render(<App />);
    expect(await screen.findByText(/las fichas se crean con el título/)).toBeDefined();
    expect(screen.getByRole("button", { name: "Sincronizar" })).toBeDefined();
  });

  it("desde el aviso se llega al asistente de IGDB", async () => {
    state.accounts = [cuentaSteam];
    state.hasIgdb = false;
    render(<App />);
    (await screen.findByRole("button", { name: "Configurar IGDB" })).click();
    expect(await screen.findByText("Metadatos: IGDB")).toBeDefined();
  });

  it("ofrece conectar GOG cuando aún no hay cuenta de GOG", async () => {
    state.accounts = [cuentaSteam];
    render(<App />);
    (await screen.findByRole("button", { name: "Conectar GOG" })).click();
    // Se busca algo que solo esté en el asistente: el encabezado se llama igual
    // que el botón que lleva hasta él y no distinguiría nada.
    expect(await screen.findByLabelText("Client ID")).toBeDefined();
    expect(screen.getByText(/Tu contraseña de GOG no pasa por aquí/)).toBeDefined();
  });

  it("con solo GOG conectado todavía se puede añadir Steam", async () => {
    // La primera pantalla solo aparece sin ninguna cuenta: quien empezara por
    // GOG se quedaba sin ninguna forma de llegar a Steam después.
    state.accounts = [cuentaGog];
    render(<App />);
    (await screen.findByRole("button", { name: "Conectar Steam" })).click();
    expect(await screen.findByLabelText("Clave de API de Steam")).toBeDefined();
  });

  it("ofrece conectar Epic cuando aún no hay cuenta de Epic", async () => {
    state.accounts = [cuentaSteam];
    render(<App />);
    (await screen.findByRole("button", { name: "Conectar Epic" })).click();
    // Se busca algo que solo esté en el asistente: el encabezado se llama igual
    // que el botón que lleva hasta él y no distinguiría nada.
    expect(await screen.findByLabelText("Client ID")).toBeDefined();
    expect(screen.getByText(/Tu contraseña de Epic no pasa por aquí/)).toBeDefined();
  });

  it("con las tres tiendas conectadas ya no ofrece conectar ninguna", async () => {
    state.accounts = [cuentaSteam, cuentaGog, cuentaEpic];
    render(<App />);
    await screen.findByRole("button", { name: "Sincronizar" });
    expect(screen.queryByRole("button", { name: "Conectar Steam" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Conectar GOG" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Conectar Epic" })).toBeNull();
  });

  it("una tienda que va bien no sale por ninguna parte", async () => {
    // El estado del conector solo se enseña cuando hay algo que contar: una
    // lista permanente de «todo bien» es ruido que nadie lee.
    state.accounts = [cuentaSteam, cuentaEpic];
    state.connectors = [{ store: "epic", enabled: true, last_error: null }];
    render(<App />);
    await screen.findByRole("button", { name: "Sincronizar" });
    expect(screen.queryByRole("button", { name: "Desactivar Epic" })).toBeNull();
  });

  it("un conector que ha fallado dice por qué y se puede desactivar", async () => {
    // Es el «done when» de la fase 7: Epic se rompe, se ve el motivo, y apagarlo
    // no toca ni Steam ni GOG.
    state.accounts = [cuentaSteam, cuentaEpic];
    state.connectors = [
      { store: "epic", enabled: true, last_error: "credenciales inválidas o caducadas" },
    ];
    render(<App />);

    expect(await screen.findByText(/credenciales inválidas o caducadas/)).toBeDefined();
    (await screen.findByRole("button", { name: "Desactivar Epic" })).click();

    expect(await screen.findByRole("button", { name: "Reactivar Epic" })).toBeDefined();
    // Y lo que importa: las demás siguen donde estaban.
    expect(screen.getByRole("button", { name: "Sincronizar" })).toBeDefined();
    expect(screen.getByText(/steam/)).toBeDefined();
  });

  it("un conector desactivado explica que lo suyo sigue en la biblioteca", async () => {
    state.accounts = [cuentaSteam, cuentaEpic];
    state.connectors = [{ store: "epic", enabled: false, last_error: null }];
    render(<App />);
    expect(await screen.findByText(/sigue en la biblioteca/)).toBeDefined();
  });

  it("sin ninguna cuenta se puede empezar por GOG en vez de por Steam", async () => {
    render(<App />);
    (await screen.findByRole("button", { name: "o empezar por GOG" })).click();
    expect(await screen.findByRole("button", { name: /Iniciar sesión en GOG/ })).toBeDefined();
  });

  it("sin ninguna cuenta también se puede empezar por Epic", async () => {
    // Quien solo tenga Epic no puede quedarse en un callejón en la primera
    // pantalla, que es justo lo que le pasaba a quien solo tenía GOG.
    render(<App />);
    (await screen.findByRole("button", { name: "o empezar por Epic" })).click();
    expect(await screen.findByRole("button", { name: /Iniciar sesión en Epic/ })).toBeDefined();
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
        cover_url: null,
        store_url: null,
        tie: false,
        candidates: [
          {
            igdb_id: 132727,
            name: "Disco Elysium: The Final Cut",
            score: 0.97,
            release_year: 2021,
            cover_url: null,
            slug: null,
          },
          {
            igdb_id: 115653,
            name: "Disco Elysium",
            score: 0.93,
            release_year: 2019,
            cover_url: null,
            slug: null,
          },
        ],
      },
    ];
    render(<App />);
    (await screen.findByRole("button", { name: /Por revisar \(1\)/ })).click();
    expect(await screen.findByText(/Disco Elysium: The Final Cut/)).toBeDefined();
    expect(screen.getByText(/crear ficha con el título de la tienda/)).toBeDefined();
  });

  it("los empates van agrupados y aparte del resto", async () => {
    // Es el motivo más común de acabar en la cola: IGDB repite fichas y las
    // ediciones se normalizan al mismo título. Agruparlos es lo que hace la
    // revisión llevadera sin tocar el umbral.
    state.accounts = [cuentaSteam];
    state.queue = [
      {
        store_entry_id: "11111111-1111-7111-8111-111111111111",
        store: "steam",
        title: "LIMBO",
        cover_url: null,
        store_url: null,
        tie: true,
        candidates: [
          { igdb_id: 1, name: "Limbo", score: 1, release_year: 2010, cover_url: null, slug: "limbo" },
          { igdb_id: 2, name: "Limbo", score: 1, release_year: 2011, cover_url: null, slug: null },
        ],
      },
      {
        store_entry_id: "22222222-2222-7222-8222-222222222222",
        store: "gog",
        title: "Otro juego",
        cover_url: null,
        store_url: null,
        tie: false,
        candidates: [
          { igdb_id: 3, name: "Otro juego", score: 0.95, release_year: 2015, cover_url: null, slug: null },
        ],
      },
    ];
    render(<App />);
    (await screen.findByRole("button", { name: /Por revisar \(2\)/ })).click();
    expect(await screen.findByText(/Empates \(1\)/)).toBeDefined();
    expect(screen.getByText(/El resto \(1\)/)).toBeDefined();
    // El año es lo que distingue dos fichas que se llaman igual.
    expect(screen.getByText(/2010/)).toBeDefined();
    // Y para las que ni así, el enlace a la ficha de IGDB. Solo aparece cuando
    // IGDB publicó un slug: sin él no hay página a la que ir.
    expect(screen.getByRole("button", { name: "Ver Limbo en IGDB" })).toBeDefined();
    expect(screen.getAllByRole("button", { name: /en IGDB$/ })).toHaveLength(1);
  });

  it("elegir candidatos ofrece confirmarlos en lote", async () => {
    state.accounts = [cuentaSteam];
    state.queue = [
      {
        store_entry_id: "11111111-1111-7111-8111-111111111111",
        store: "steam",
        title: "LIMBO",
        cover_url: null,
        store_url: null,
        tie: true,
        candidates: [
          { igdb_id: 1, name: "Limbo", score: 1, release_year: 2010, cover_url: null, slug: "limbo" },
          { igdb_id: 2, name: "Limbo", score: 1, release_year: 2011, cover_url: null, slug: null },
        ],
      },
    ];
    render(<App />);
    (await screen.findByRole("button", { name: /Por revisar \(1\)/ })).click();
    // Sin nada elegido no hay botón de lote: nada que confirmar.
    expect(screen.queryByRole("button", { name: /Confirmar 1 emparejamiento/ })).toBeNull();
    (await screen.findByRole("button", { name: /Limbo · 2010/ })).click();
    expect(
      await screen.findByRole("button", { name: /Confirmar 1 emparejamiento/ }),
    ).toBeDefined();
  });
});
