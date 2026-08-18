import { describe, expect, it, mock, beforeEach } from "bun:test";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import type {
  Account,
  AppInfo,
  ConnectorState,
  LibraryRow,
  LibrarySummary,
  PlayStatus,
  PriceRow,
  ReviewItem,
  StateUpdate,
} from "./lib/api";

const state = {
  info: { version: "0.1.0", secrets_backend: "keyring", unlocked: true } as AppInfo,
  accounts: [] as Account[],
  connectors: [] as ConnectorState[],
  hasIgdb: true,
  hasItad: true,
  summary: { owned: 0, wishlist: 0, games: 0, pending_review: 0 } as LibrarySummary,
  queue: [] as ReviewItem[],
  rows: [] as LibraryRow[],
  prices: [] as PriceRow[],
  /** What was really written, so that you can count it and look at it. */
  saved: [] as [string, PlayStatus | null, number | null, string | null][],
  /** How many commands the writes took: a batch of thirty must be one. */
  saveCalls: 0,
  /** How many times all of the library was asked for. A save must not add one. */
  libraryRequests: 0,
  /** The matches that the batch confirmed. */
  confirmed: [] as [string, number][],
  /** How many times the prices were really requested. */
  priceRequests: 0,
  /** The reason that the matching stopped, if it stopped. */
  matchingStopped: null as string | null,
};

/**
 * What Rust does with a save: it writes and gives the rows back already made.
 *
 * The rows of `state.rows` are written too, because that is what the database
 * does: a later `library()` must not deny what the save answered.
 */
function write(updates: StateUpdate[]): LibraryRow[] {
  return updates.map((update) => {
    const saved: LibraryRow = {
      ...state.rows.find((row) => row.game_id === update.gameId)!,
      status: update.status,
      rating: update.rating,
      notes: update.notes,
    };
    state.saved.push([update.gameId, update.status, update.rating, update.notes]);
    state.rows = state.rows.map((row) => (row.game_id === update.gameId ? saved : row));
    return saved;
  });
}

// The event bus of Tauri does not exist out of the application window.
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
    hasItadCredentials: () => Promise.resolve(state.hasItad),
    setItadCredentials: () => Promise.resolve(),
    prices: () => Promise.resolve(state.prices),
    refreshPrices: () => {
      state.priceRequests += 1;
      return Promise.resolve({ priced: 0, unknown: 0, cancelled: false });
    },
    librarySummary: () => Promise.resolve(state.summary),
    reviewQueue: () => Promise.resolve(state.queue),
    syncNow: () =>
      Promise.resolve({ owned: 0, wishlist: 0, removed: 0, failures: [], skipped: [] }),
    resolveIdentities: () =>
      Promise.resolve({
        linked: 0,
        review: 0,
        unknown: 0,
        cancelled: false,
        stopped: state.matchingStopped,
      }),
    unlockSecrets: () => Promise.resolve(),
    connectSteam: () => Promise.resolve("id"),
    connectGog: () => Promise.resolve("id"),
    connectEpic: () => Promise.resolve("id"),
    setIgdbCredentials: () => Promise.resolve(),
    reviewConfirm: () => Promise.resolve(),
    reviewConfirmMany: (decisions: [string, number][]) => {
      state.confirmed.push(...decisions);
      return Promise.resolve(decisions.length);
    },
    reviewWithoutMetadata: () => Promise.resolve(),
    library: () => {
      state.libraryRequests += 1;
      return Promise.resolve(state.rows);
    },
    cancelOperation: () => Promise.resolve(),
    setUserState: (
      gameId: string,
      status: PlayStatus | null,
      rating: number | null,
      notes: string | null,
    ) => {
      state.saveCalls += 1;
      return Promise.resolve(write([{ gameId, status, rating, notes }])[0]!);
    },
    setUserStateMany: (updates: StateUpdate[]) => {
      state.saveCalls += 1;
      return Promise.resolve(write(updates));
    },
  },
  errorMessage: (cause: unknown) => String(cause),
}));

const { App } = await import("./App");

const steamAccount: Account = {
  store: "steam",
  account_ref: "7656119",
  display_name: "serjor",
  last_sync_at: null,
};

const gogAccount: Account = {
  store: "gog",
  account_ref: "51000000000000000",
  display_name: "serjor",
  last_sync_at: null,
};

const epicAccount: Account = {
  store: "epic",
  account_ref: "a1b2c3d4e5f64788b0c1d2e3f4a5b6c7",
  display_name: "serjor",
  last_sync_at: null,
};

function row(overrides: Partial<LibraryRow>): LibraryRow {
  return {
    game_id: crypto.randomUUID(),
    title: "Game",
    sort_title: "game",
    cover_url: null,
    summary: null,
    release_year: null,
    genres: [],
    owned_stores: ["steam"],
    wishlist_stores: [],
    store_cover_url: null,
    store_url: null,
    playtime_minutes: 0,
    last_played_at: null,
    status: null,
    rating: null,
    notes: null,
    ...overrides,
  };
}

/**
 * Four rows and no more in the table tests: with no real layout, the virtual
 * list measures the container as zero and shows only its reserve window. Four is
 * what fits, and it is also the batch size that the plan asks for.
 */
const FOUR = [
  row({ title: "Celeste", sort_title: "celeste", playtime_minutes: 900, rating: 9, notes: "short and complete" }),
  row({ title: "Hades", sort_title: "hades", playtime_minutes: 3120, rating: 8 }),
  row({ title: "Outer Wilds", sort_title: "outer wilds", playtime_minutes: 0 }),
  row({ title: "Prey", sort_title: "prey", playtime_minutes: 120, rating: 6 }),
];

/**
 * A library with material for three shelves of "Today" at the same time: two
 * started, one never started and one with a copy in two stores. The dates are
 * relative to the clock because the limits of "Today" are also relative.
 */
const NOW = Math.floor(Date.now() / 1000);
const DAY = 86_400;
const FOR_TODAY = [
  row({ title: "Hades", sort_title: "hades", status: "playing", playtime_minutes: 3120, last_played_at: NOW - 2 * DAY }),
  row({ title: "Celeste", sort_title: "celeste", status: "playing", playtime_minutes: 900, last_played_at: NOW - 100 * DAY }),
  row({ title: "Prey", sort_title: "prey", owned_stores: ["steam", "gog"], playtime_minutes: 120, last_played_at: NOW - 10 * DAY }),
  row({ title: "Outer Wilds", sort_title: "outer wilds" }),
];

/**
 * Three wished-for games: one at its all-time low, one with any other discount
 * and one that nobody sells. They are the three conditions of the screen.
 */
const WISHES = [
  row({ title: "Blasphemous", sort_title: "blasphemous", owned_stores: [], wishlist_stores: ["gog"] }),
  row({ title: "Silksong", sort_title: "silksong", owned_stores: [], wishlist_stores: ["steam"] }),
  row({ title: "Tunic", sort_title: "tunic", owned_stores: [], wishlist_stores: ["steam"] }),
];

function price(game_id: string, overrides: Partial<PriceRow> = {}): PriceRow {
  return {
    game_id,
    shop: "GOG",
    amount: 1599,
    regular: 3999,
    cut: 60,
    currency: "EUR",
    shops: 2,
    low_all_time: 899,
    low_year: 1349,
    itad_slug: "a-game",
    captured_at: 1_755_000_000,
    ...overrides,
  };
}

const checkOf = (title: string) => screen.getByLabelText(`Select ${title}`) as HTMLInputElement;

/**
 * Which game the open record belongs to, beside the table or on top of the
 * covers.
 *
 * By the identifier to which `aria-labelledby` points, and not by "the second
 * heading of the screen": "Today" also has one for its proposal.
 */
const recordTitle = () => document.getElementById("card-title");
const openRecord = () => recordTitle()?.textContent ?? null;

/**
 * In the record and not in all of the screen: the filter of the bar is also
 * called "Status", and a search by name alone finds the two.
 */
const inTheRecord = () => within(recordTitle()!.closest(".card") as HTMLElement);

/** Two records with the same score: the most common reason to come to the
 *  queue. */
const TIE: ReviewItem = {
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
};

/** An entry in which one candidate wins clearly: it comes already selected. */
const CLEAR: ReviewItem = {
  store_entry_id: "22222222-2222-7222-8222-222222222222",
  store: "gog",
  title: "Another game",
  cover_url: null,
  store_url: null,
  tie: false,
  candidates: [
    { igdb_id: 3, name: "Another game", score: 0.95, release_year: 2015, cover_url: null, slug: null },
    {
      igdb_id: 4,
      name: "Another game, but of 2009",
      score: 0.72,
      release_year: 2009,
      cover_url: null,
      slug: null,
    },
  ],
};

/**
 * What the "will match with" column of an entry says.
 *
 * The search uses the store title in its own cell, and not the title alone: the
 * chosen candidate can have the same name as the entry — in fact that is usual
 * when the matching is correct — and then the title appears two times.
 */
const matchesWith = (title: string) =>
  screen.getByText(title, { selector: ".source strong" }).closest("tr")?.cells[2]?.textContent;

/**
 * The width of the window is what decides between the inspector and the sheet,
 * thus this file must be able to change it: happy-dom gives it and `matchMedia`
 * obeys it.
 */
function width(px: number) {
  (
    window as unknown as { happyDOM: { setViewport: (v: { width: number }) => void } }
  ).happyDOM.setViewport({ width: px });
}

describe("App", () => {
  beforeEach(() => {
    // Wide by default: that is where all of the library is visible, and the
    // narrow window is tested separately.
    width(1400);
    state.info = { version: "0.1.0", secrets_backend: "keyring", unlocked: true };
    state.accounts = [];
    state.connectors = [];
    state.hasIgdb = true;
    state.hasItad = true;
    state.summary = { owned: 0, wishlist: 0, games: 0, pending_review: 0 };
    state.queue = [];
    state.rows = [];
    state.prices = [];
    state.saved = [];
    state.saveCalls = 0;
    state.libraryRequests = 0;
    state.confirmed = [];
    state.priceRequests = 0;
    state.matchingStopped = null;
  });

  it("a matching that stops says why and that the work is kept", async () => {
    // It stops, it does not fail: it writes its work and gives back the reason.
    // Without that message, the user sees incomplete work and does not know why.
    state.accounts = [steamAccount];
    state.matchingStopped = "the request limit is reached";
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Match" }));

    const message = await screen.findByRole("alert");
    expect(message.textContent).toContain("the request limit is reached");
    expect(message.textContent).toContain("is kept");
  });

  it("with no connected account it goes to the Steam setup screen", async () => {
    render(<App />);
    expect(await screen.findByText("Connect Steam")).toBeDefined();
  });

  it("with no keyring in the system it asks for the passphrase first", async () => {
    state.info = { version: "0.1.0", secrets_backend: "passphrase", unlocked: false };
    render(<App />);
    expect(await screen.findByText("Passphrase of the store")).toBeDefined();
  });

  it("with no IGDB it gives a message and does not block the library", async () => {
    // The record comes from the matching, thus with no IGDB it comes from the
    // title of the store. It is a message, not an error: to block all of the
    // application until the user has Twitch credentials is too hard at the first
    // start.
    state.accounts = [steamAccount];
    state.hasIgdb = false;
    render(<App />);
    expect(await screen.findByText(/the records are made with the title/)).toBeDefined();
    expect(screen.getByRole("button", { name: "Synchronise" })).toBeDefined();
  });

  it("from the message you reach the IGDB setup screen", async () => {
    state.accounts = [steamAccount];
    state.hasIgdb = false;
    render(<App />);
    (await screen.findByRole("button", { name: "Configure IGDB" })).click();
    expect(await screen.findByText("Metadata: IGDB")).toBeDefined();
  });

  it("it offers to connect GOG when there is no GOG account yet", async () => {
    state.accounts = [steamAccount];
    render(<App />);
    (await screen.findByRole("button", { name: "Connect GOG" })).click();
    // The search uses something that is only in the setup screen: the heading has
    // the same name as the button that goes to it and would tell nothing apart.
    expect(await screen.findByLabelText("Client ID")).toBeDefined();
    expect(screen.getByText(/Your GOG password does not come through here/)).toBeDefined();
  });

  it("with only GOG connected you can still add Steam", async () => {
    // The first screen appears only with no account: a user who started with GOG
    // had no way to reach Steam later.
    state.accounts = [gogAccount];
    render(<App />);
    (await screen.findByRole("button", { name: "Connect Steam" })).click();
    expect(await screen.findByLabelText("Steam API key")).toBeDefined();
  });

  it("it offers to connect Epic when there is no Epic account yet", async () => {
    state.accounts = [steamAccount];
    render(<App />);
    (await screen.findByRole("button", { name: "Connect Epic" })).click();
    // The search uses something that is only in the setup screen: the heading has
    // the same name as the button that goes to it and would tell nothing apart.
    expect(await screen.findByLabelText("Client ID")).toBeDefined();
    expect(screen.getByText(/Your Epic password does not come through here/)).toBeDefined();
  });

  it("with the three stores connected it offers to connect none", async () => {
    state.accounts = [steamAccount, gogAccount, epicAccount];
    render(<App />);
    await screen.findByRole("button", { name: "Synchronise" });
    expect(screen.queryByRole("button", { name: "Connect Steam" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Connect GOG" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Connect Epic" })).toBeNull();
  });

  it("a store that operates correctly appears in no place", async () => {
    // The state of the connector is shown only when there is something to say: a
    // permanent list of "all correct" is noise that nobody reads.
    state.accounts = [steamAccount, epicAccount];
    state.connectors = [{ store: "epic", enabled: true, last_error: null }];
    render(<App />);
    await screen.findByRole("button", { name: "Synchronise" });
    expect(screen.queryByRole("button", { name: "Switch Epic off" })).toBeNull();
  });

  it("a connector that failed says why and you can switch it off", async () => {
    // It is the "done when" of phase 7: Epic breaks, you see the reason, and to
    // switch it off touches neither Steam nor GOG.
    state.accounts = [steamAccount, epicAccount];
    state.connectors = [
      { store: "epic", enabled: true, last_error: "invalid or expired credentials" },
    ];
    render(<App />);

    expect(await screen.findByText(/invalid or expired credentials/)).toBeDefined();
    (await screen.findByRole("button", { name: "Switch Epic off" })).click();

    expect(await screen.findByRole("button", { name: "Switch Epic on" })).toBeDefined();
    // And what is important: the others stay where they were.
    expect(screen.getByRole("button", { name: "Synchronise" })).toBeDefined();
    expect(screen.getByText(/steam/)).toBeDefined();
  });

  it("a connector switched off explains that its data stays in the library", async () => {
    state.accounts = [steamAccount, epicAccount];
    state.connectors = [{ store: "epic", enabled: false, last_error: null }];
    render(<App />);
    expect(await screen.findByText(/stays in the library/)).toBeDefined();
  });

  it("with no account you can start with GOG and not with Steam", async () => {
    render(<App />);
    (await screen.findByRole("button", { name: "or start with GOG" })).click();
    expect(await screen.findByRole("button", { name: /Sign in to GOG/ })).toBeDefined();
  });

  it("with no account you can also start with Epic", async () => {
    // A user who has only Epic cannot be in a dead end on the first screen, which
    // is exactly what occurred to a user who had only GOG.
    render(<App />);
    (await screen.findByRole("button", { name: "or start with Epic" })).click();
    expect(await screen.findByRole("button", { name: /Sign in to Epic/ })).toBeDefined();
  });

  it("it shows the count of records, copies and games to review", async () => {
    state.accounts = [steamAccount];
    state.summary = { owned: 412, wishlist: 37, games: 400, pending_review: 12 };
    render(<App />);
    expect(await screen.findByText(/400 records/)).toBeDefined();
    expect(screen.getByText(/12 to review/)).toBeDefined();
  });

  it("the library shows the records with their stores", async () => {
    state.accounts = [steamAccount];
    state.rows = [
      row({
        title: "Disco Elysium",
        sort_title: "disco elysium",
        release_year: 2019,
        genres: ["RPG"],
        owned_stores: ["steam", "gog"],
        playtime_minutes: 1240,
      }),
    ];
    render(<App />);
    // Each game is one row, and in it are the two stores that have it: that is
    // what tells a duplicate copy from one copy.
    const cell = await screen.findByRole("button", { name: "Disco Elysium" });
    const rowDom = cell.closest("tr");
    expect(rowDom?.textContent).toContain("steam");
    expect(rowDom?.textContent).toContain("gog");
    expect(rowDom?.textContent).toContain("21 h");
  });

  it("a click on a column sorts by it, and a second click inverts it", async () => {
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);

    const titles = () =>
      screen.getAllByRole("button").filter((b) => b.className === "cell").map((b) => b.textContent);

    // At the start, by title.
    expect(await screen.findByRole("button", { name: "Celeste" })).toBeDefined();
    expect(titles()).toEqual(["Celeste", "Hades", "Outer Wilds", "Prey"]);

    fireEvent.click(screen.getByRole("button", { name: /Hours/ }));
    // Ascending, and the games not played last even if their value is zero.
    expect(titles()).toEqual(["Prey", "Celeste", "Hades", "Outer Wilds"]);

    fireEvent.click(screen.getByRole("button", { name: /Hours/ }));
    expect(titles()).toEqual(["Hades", "Celeste", "Prey", "Outer Wilds"]);
  });

  it("with the shift key it selects all of the range, not one row", async () => {
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });

    fireEvent.click(checkOf("Celeste"));
    expect(checkOf("Celeste").checked).toBe(true);
    expect(checkOf("Prey").checked).toBe(false);

    fireEvent.click(checkOf("Prey"), { shiftKey: true });
    for (const title of ["Celeste", "Hades", "Outer Wilds", "Prey"]) {
      expect(checkOf(title).checked).toBe(true);
    }
  });

  it("a change of view does not change which games are in front of you", async () => {
    // The filter and the sort are applied in one place and the two views show the
    // result. The test is that there are not two places to become different.
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });

    const set = () =>
      screen
        .getAllByLabelText(/^Select /)
        .map((e) => e.getAttribute("aria-label"))
        .sort();

    fireEvent.change(screen.getByPlaceholderText("Search in the library"), {
      target: { value: "out" },
    });
    expect(set()).toEqual(["Select Outer Wilds"]);

    fireEvent.click(screen.getByRole("button", { name: "Covers" }));
    expect(set()).toEqual(["Select Outer Wilds"]);
    // And it is still the wall, not the table in different clothes.
    expect(screen.queryByRole("columnheader")).toBeNull();
  });

  it("what is selected in the table stays selected in the covers", async () => {
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });

    fireEvent.click(checkOf("Celeste"));
    fireEvent.click(checkOf("Hades"), { shiftKey: true });

    fireEvent.click(screen.getByRole("button", { name: "Covers" }));

    expect(checkOf("Celeste").checked).toBe(true);
    expect(checkOf("Hades").checked).toBe(true);
    expect(checkOf("Prey").checked).toBe(false);
    // The bulk bar does not see that the view changed.
    expect(screen.getByText("2 selected")).toBeDefined();
  });

  it("the batch writes one time for each game and does not delete the text", async () => {
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });
    const asked = state.libraryRequests;

    fireEvent.click(checkOf("Celeste"));
    fireEvent.click(checkOf("Prey"), { shiftKey: true });

    fireEvent.change(screen.getByLabelText("Mark as"), { target: { value: "abandoned" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));

    // One write for each game, and no more: the batch cannot write two times on
    // the same game and cannot miss one. And all of them in **one** command:
    // four games were four commands, one after another.
    await waitFor(() => expect(state.saved).toHaveLength(4));
    expect(state.saveCalls).toBe(1);
    expect(state.libraryRequests).toBe(asked);
    expect(state.saved.every(([, status]) => status === "abandoned")).toBe(true);
    // The rating and the notes are given back unchanged: `set_user_state` writes
    // all of the row again, and without this a bulk change of status would
    // quietly delete the only data that the application knows about the user.
    const celeste = state.saved.find(([id]) => id === FOUR[0]!.game_id);
    expect(celeste?.[2]).toBe(9);
    expect(celeste?.[3]).toBe("short and complete");
  });

  it("from the table the record opens beside it, and ↑↓ goes through the list", async () => {
    // It is the reason that the inspector exists: to compare games one at a time
    // without you go back to the table to find the next one and lose the record.
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));

    expect(openRecord()).toBe("Celeste");
    expect(screen.queryByRole("dialog")).toBeNull();

    fireEvent.keyDown(window, { key: "ArrowDown" });
    expect(openRecord()).toBe("Hades");
    fireEvent.keyDown(window, { key: "ArrowUp" });
    expect(openRecord()).toBe("Celeste");

    // And at the end it does not close and does not go to the other end: it
    // stays where it is.
    fireEvent.keyDown(window, { key: "ArrowUp" });
    expect(openRecord()).toBe("Celeste");
  });

  it("while you write a note, ↑↓ moves the cursor and not the game", async () => {
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));

    fireEvent.keyDown(inTheRecord().getByLabelText("Notes"), { key: "ArrowDown" });
    expect(openRecord()).toBe("Celeste");
  });

  it("in a narrow window the record of the table opens as a sheet", async () => {
    // Below its range the inspector does not fit beside the table, and to keep it
    // there would cut the title exactly when you compare records.
    width(1000);
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));

    expect(screen.getByRole("dialog")).toBeDefined();
    expect(openRecord()).toBe("Celeste");
  });

  it("from the covers the record opens as a sheet, with the store art", async () => {
    state.accounts = [steamAccount];
    state.rows = [
      row({
        title: "Celeste",
        sort_title: "celeste",
        summary: "Help Madeline to survive her own demons.",
        store_cover_url: "https://cdn.cloudflare.steamstatic.com/steam/apps/504230/header.jpg",
      }),
    ];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Covers" }));
    fireEvent.click(screen.getByRole("button", { name: /^Celeste/ }));

    expect(screen.getByRole("dialog")).toBeDefined();
    expect(screen.getByText(/survive her own demons/)).toBeDefined();
    // Decoration — the title is below — thus it has no role and the search uses
    // what it is: the wide image of the store, which is what the sheet adds.
    const art = document.querySelector(".sheet-art");
    expect(art?.tagName).toBe("IMG");
    expect(art?.getAttribute("src")).toContain("header.jpg");
  });

  it("with no store cover and no summary the sheet still opens", async () => {
    // The condition of a user who has not configured IGDB, which is what the
    // message in the header promises: the record has less to show, not less to
    // do.
    width(1000);
    state.accounts = [steamAccount];
    state.rows = [row({ title: "Celeste", sort_title: "celeste" })];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));

    expect(screen.getByRole("dialog")).toBeDefined();
    expect(screen.getByText(/No summary/)).toBeDefined();
    expect(document.querySelector(".sheet-art")?.tagName).toBe("DIV");
    expect(inTheRecord().getByLabelText("Notes")).toBeDefined();
  });

  it("a save from the sheet writes the same as a save from the inspector", async () => {
    // One form and one save: the presentation cannot change what reaches the
    // database.
    state.accounts = [steamAccount];
    state.rows = [row({ title: "Celeste", sort_title: "celeste", rating: 9 })];

    for (const [px, expected] of [
      [1400, null],
      [1000, "dialog"],
    ] as const) {
      width(px);
      const { unmount } = render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));
      expect(screen.queryByRole("dialog") === null ? null : "dialog").toBe(expected);

      const id = state.rows[0]!.game_id;
      fireEvent.change(inTheRecord().getByLabelText("Status"), { target: { value: "finished" } });
      fireEvent.click(inTheRecord().getByRole("button", { name: "Save" }));
      await waitFor(() => expect(state.saved).toHaveLength(1));

      expect(state.saved[0]).toEqual([id, "finished", 9, null]);
      state.saved = [];
      unmount();
    }
  });

  it("a save changes the row on the screen and does not ask for the library", async () => {
    // The save answers with its row already made. Before, the interface answered
    // a status with a complete refresh — all of the library, all of the review
    // queue and all of the prices — to see one word change on one row.
    state.accounts = [steamAccount];
    state.rows = [row({ title: "Celeste", sort_title: "celeste" })];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));
    const asked = state.libraryRequests;

    fireEvent.change(inTheRecord().getByLabelText("Status"), { target: { value: "playing" } });
    fireEvent.click(inTheRecord().getByRole("button", { name: "Save" }));

    // The table shows the new status, thus the row that came back reached the
    // state and the screen is not waiting for a later load. In the row of the
    // table and not anywhere: the form of the record also says "Playing", and
    // that says nothing about the list.
    await waitFor(() =>
      expect(
        within(screen.getByRole("row", { name: /Celeste/ })).getByText("Playing"),
      ).toBeDefined(),
    );
    expect(state.saveCalls).toBe(1);
    expect(state.libraryRequests).toBe(asked);
  });

  it('"Today" proposes the game half done and does not repeat it on the shelves', async () => {
    // To propose something new while you have a game started is what makes the
    // pile grow, which is exactly what this screen tries to undo.
    state.accounts = [steamAccount];
    state.rows = FOR_TODAY;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Today" }));

    expect((await screen.findByRole("heading", { level: 2 })).textContent).toBe("Hades");
    expect(screen.getByText("You have it half done")).toBeDefined();

    // Each shelf with its reason, and only the shelves that have something in
    // them: "you have not touched it for a long time" does not appear because
    // nothing reaches six months.
    expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual([
      "You stopped in the middle",
      "Never started",
      "You have it two times",
    ]);

    // And the featured game does not appear again below: to see it two times on
    // the same screen makes you think that they are two games.
    expect(screen.queryAllByRole("button", { name: /^Hades/ })).toHaveLength(0);
    expect(screen.getByRole("button", { name: /^Celeste/ })).toBeDefined();
  });

  it('"Today" shows no empty shelf and does not break with an empty library', async () => {
    state.accounts = [steamAccount];
    state.rows = [];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Today" }));

    expect(await screen.findByText(/There is not yet an owned game/)).toBeDefined();
    expect(screen.queryByRole("heading", { level: 3 })).toBeNull();
  });

  it('"Today" makes its own divisions and does not take the library filters', async () => {
    // It is what separates "Today" from a third view mode: the table and the wall
    // share a contract — you filter and the two show the filtered games — and
    // this screen makes its own proposal.
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });

    fireEvent.change(screen.getByPlaceholderText("Search in the library"), {
      target: { value: "celeste" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Today" }));

    expect(screen.getByRole("heading", { level: 2 }).textContent).toBe("Outer Wilds");
  });

  it('from "Today" the record opens as a sheet even in a wide window', async () => {
    // Here there is no list beside it to keep in view, and what you look at is
    // the art.
    state.accounts = [steamAccount];
    state.rows = FOUR;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Today" }));
    fireEvent.click(screen.getByRole("button", { name: "Open the record" }));

    expect(screen.getByRole("dialog")).toBeDefined();
    expect(openRecord()).toBe("Outer Wilds");
  });

  it("the wished-for games come sorted by discount, with the all-time low beside", async () => {
    // It is the "done when" of phase 8: a −60 % means nothing alone, and a sort
    // by title turns the list into good intentions.
    state.accounts = [steamAccount];
    state.rows = WISHES;
    state.prices = [
      price(WISHES[0]!.game_id, { cut: 40, amount: 2399 }),
      price(WISHES[1]!.game_id, { cut: 75, amount: 899, shop: "Steam" }),
    ];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /^Wishlist/ }));

    const titles = () =>
      screen.getAllByText(/./, { selector: ".wish-title" }).map((e) => e.textContent);
    // The largest discount first, and the games with no price last: that is not
    // "inexpensive", it is that there is no data.
    expect(titles()).toEqual(["Silksong", "Blasphemous", "Tunic"]);

    const silksong = screen.getByText("Silksong").closest("tr");
    expect(silksong?.textContent).toContain("8.99");
    expect(silksong?.textContent).toContain("−75%");
    expect(silksong?.textContent).toContain("Steam");
    // It is at its all-time low, which is the only question that a person who
    // looks at this screen asks.
    expect(silksong?.textContent).toContain("at its low");

    const blasphemous = screen.getByText("Blasphemous").closest("tr");
    expect(blasphemous?.textContent).not.toContain("at its low");
    expect(screen.getByText("Tunic").closest("tr")?.textContent).toContain("no price");
  });

  it("an owned game does not come into the wishlist", async () => {
    state.accounts = [steamAccount];
    state.rows = [...WISHES, row({ title: "Hades", sort_title: "hades" })];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /^Wishlist/ }));

    expect(screen.queryByText("Hades")).toBeNull();
  });

  it("with no ITAD key the list continues, and from it you reach the setup", async () => {
    // The same treatment as IGDB: with no key there is less to show, not less to
    // do.
    state.accounts = [steamAccount];
    state.hasItad = false;
    state.rows = WISHES;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /^Wishlist/ }));

    expect(screen.getByText("Silksong")).toBeDefined();
    expect(screen.queryByRole("button", { name: "Update the prices" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Configure ITAD" }));
    expect(await screen.findByText("Prices: IsThereAnyDeal")).toBeDefined();
  });

  it("with no wished-for game you can still configure ITAD", async () => {
    // The key is kept before there are wished-for games, not after: to hide the
    // link until there was a list left a user who had just installed the
    // application with no way to configure it.
    state.accounts = [steamAccount];
    state.hasItad = false;
    state.rows = [];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /^Wishlist/ }));

    expect(screen.getByText(/There is no game in your wishlist/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Configure ITAD" }));
    expect(await screen.findByText("Prices: IsThereAnyDeal")).toBeDefined();
  });

  it("with wished-for games and no record it says why the screen is empty", async () => {
    // The condition that confuses: the header says "84 wished for" because it
    // counts copies, and this screen shows records. With no explanation, it looks
    // like a defect.
    state.accounts = [steamAccount];
    state.summary = { owned: 0, wishlist: 84, games: 0, pending_review: 84 };
    state.rows = [];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /^Wishlist/ }));

    expect(screen.getByText(/84 wished-for copies/)).toBeDefined();
    expect(screen.getByText(/Click/)).toBeDefined();
  });

  it("with no wished-for game it does not offer to update prices that do not exist", async () => {
    state.accounts = [steamAccount];
    state.rows = [];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /^Wishlist/ }));

    expect(screen.queryByRole("button", { name: "Update the prices" })).toBeNull();
  });

  it("an update of the prices asks for the prices and synchronises nothing", async () => {
    state.accounts = [steamAccount];
    state.rows = WISHES;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /^Wishlist/ }));
    fireEvent.click(screen.getByRole("button", { name: "Update the prices" }));

    await waitFor(() => expect(state.priceRequests).toBe(1));
  });

  it("what wins clearly comes already selected, and what is equal does not", async () => {
    // The difference is all of the queue: to repeat with a click what the screen
    // already says is work that nobody needs, and to select for the user in a tie
    // is exactly what the threshold refused to do.
    state.accounts = [steamAccount];
    state.queue = [TIE, CLEAR];
    render(<App />);
    (await screen.findByRole("button", { name: /To review \(2\)/ })).click();

    await screen.findByText("Equal scores (1)");
    expect(matchesWith("LIMBO")).toBe("not chosen");
    expect(matchesWith("Another game")).toContain("Another game");
    // And the batch takes only the entry that comes selected.
    expect(screen.getByRole("button", { name: /Confirm 1 match$/ })).toBeDefined();
  });

  it("the batch writes what the column shows, not what was touched", async () => {
    state.accounts = [steamAccount];
    state.queue = [CLEAR];
    render(<App />);
    (await screen.findByRole("button", { name: /To review \(1\)/ })).click();

    // The other record is selected: the chosen one and the other one exchange.
    fireEvent.click(await screen.findByRole("button", { name: /^Another game, but of 2009/ }));
    expect(matchesWith("Another game")).toContain("Another game, but of 2009");

    fireEvent.click(screen.getByRole("button", { name: /Confirm 1 match$/ }));
    await waitFor(() => expect(state.confirmed).toHaveLength(1));
    expect(state.confirmed[0]).toEqual([CLEAR.store_entry_id, 4]);
  });

  it("to remove the record that came selected leaves the entry out of the batch", async () => {
    // It is the only way to say "not this one" without you say which one, and it
    // is necessary to leave one entry unresolved while the others are confirmed.
    state.accounts = [steamAccount];
    state.queue = [CLEAR];
    render(<App />);
    (await screen.findByRole("button", { name: /To review \(1\)/ })).click();

    // The chosen candidate comes with no year and no similarity — the two have a
    // column of their own — thus its accessible name is the title alone and it is
    // not confused with the other one.
    fireEvent.click(await screen.findByRole("button", { name: "Another game" }));
    expect(matchesWith("Another game")).toBe("not chosen");
    expect(screen.queryByRole("button", { name: /Confirm/ })).toBeNull();
  });

  it("the review queue offers the candidates and the way out with no record", async () => {
    state.accounts = [steamAccount];
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
    (await screen.findByRole("button", { name: /To review \(1\)/ })).click();
    expect(await screen.findByText(/Disco Elysium: The Final Cut/)).toBeDefined();
    expect(screen.getByText(/make a record with the title of the store/)).toBeDefined();
  });

  it("the equal scores come in a group and apart from the others", async () => {
    // It is the most common reason to come to the queue: IGDB repeats records and
    // the editions normalise to the same title. To group them is what makes the
    // review acceptable with no change to the threshold.
    state.accounts = [steamAccount];
    state.queue = [TIE, CLEAR];
    render(<App />);
    (await screen.findByRole("button", { name: /To review \(2\)/ })).click();
    expect(await screen.findByText(/Equal scores \(1\)/)).toBeDefined();
    expect(screen.getByText(/The remainder \(1\)/)).toBeDefined();
    // The year is what tells two records with the same name apart.
    expect(screen.getByText(/2010/)).toBeDefined();
    // And for the records that even the year does not separate, the link to the
    // IGDB record. It appears only when IGDB published a slug: without it there
    // is no page to go to.
    expect(screen.getByRole("button", { name: "See Limbo in IGDB" })).toBeDefined();
    expect(screen.getAllByRole("button", { name: /in IGDB$/ })).toHaveLength(1);
  });

  it("to select candidates offers to confirm them in a batch", async () => {
    state.accounts = [steamAccount];
    state.queue = [TIE];
    render(<App />);
    (await screen.findByRole("button", { name: /To review \(1\)/ })).click();
    // With nothing selected there is no batch button: there is nothing to
    // confirm.
    expect(screen.queryByRole("button", { name: /Confirm 1 match/ })).toBeNull();
    (await screen.findByRole("button", { name: /Limbo · 2010/ })).click();
    expect(await screen.findByRole("button", { name: /Confirm 1 match/ })).toBeDefined();
  });
});
