/**
 * The real application, in a browser, with no Tauri.
 *
 * It serves `dist/` and opens Chromium with the Tauri bridge replaced: `invoke`
 * of `@tauri-apps/api/core` calls `window.__TAURI_INTERNALS__`, thus it is
 * sufficient to put there an object with pretend answers before the bundle
 * loads. No line of the project changes and no mock stays inside `src`.
 *
 * The purpose: to **measure** the interface, not to look at it. A `bun test`
 * with happy-dom makes no layout — it measures all of the containers as zero —
 * thus it cannot say whether two covers cover each other, whether a header stops
 * agreeing with its column or whether a text goes out of its box. Only a real
 * layout engine knows that, and to look at a screenshot is misleading: in the
 * session that wrote this, three "defects" seen in screenshots did not exist and
 * one that you could not see did.
 *
 * CI runs it on Ubuntu after it installs Chromium. You can also run it by hand.
 *
 *     bun run build && bun run visual
 *
 * It finds Chromium alone. If it does not find one:
 *
 *     bunx playwright install chromium      # or
 *     CHROMIUM_PATH=/usr/bin/chromium bun run visual
 */
import { chromium, type Browser, type Page } from "playwright-core";
import type {
  Account,
  AppInfo,
  ConnectorState,
  LibraryRow,
  LibrarySummary,
  PriceRow,
  ReviewItem,
} from "../../src/lib/api";

/**
 * What Rust would answer. Each key is the name of a Tauri command.
 *
 * **All** of the commands that the application asks for at the start must be
 * here. A command that is absent gets `null`, and a `null` where the code
 * expects a list breaks the first render: what you then see is not a readable
 * error but that no test finds the screen and all of them use their thirty
 * seconds of waiting time.
 */
export interface Answers {
  app_info: AppInfo;
  list_accounts: Account[];
  connector_states: ConnectorState[];
  has_igdb_credentials: boolean;
  has_itad_credentials: boolean;
  library_summary: LibrarySummary;
  review_queue: ReviewItem[];
  library: LibraryRow[];
  prices: PriceRow[];
}

/**
 * A wide header with the ratio of the Steam header (460×215), served from the
 * page itself: what the tests measure is the box in which it is cut, and for
 * that the CDN and a configuration are unnecessary.
 */
export const WIDE_ART =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    "<svg xmlns='http://www.w3.org/2000/svg' width='460' height='215'>" +
      "<rect width='460' height='215' fill='gray'/></svg>",
  );

export const PORTRAIT_ART =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    "<svg xmlns='http://www.w3.org/2000/svg' width='150' height='200'>" +
      "<rect width='150' height='200' fill='gray'/>" +
      "<circle cx='75' cy='100' r='48' fill='white'/></svg>",
  );

export const LONG_PROVIDER_ERROR = "Corrective action is required to continue. Open this Epic page and connect the account again: https://www.epicgames.com/id/login/continuation?code=example";

export async function openUtilities(page: Page): Promise<void> {
  await page.getByRole("button", { name: "Utilities" }).click();
  await page.locator("dialog[open]").waitFor();
}

export function game(overrides: Partial<LibraryRow> = {}): LibraryRow {
  const title = overrides.title ?? "Game";
  return {
    game_id: crypto.randomUUID(),
    title,
    sort_title: title.toLowerCase(),
    cover_url: null,
    summary: null,
    release_year: 2020,
    genres: ["RPG"],
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
 * A library with the difficult conditions in it: titles that do not fit in one
 * line, games in two stores, games never started, games with no status and games
 * with no note. The easy conditions break nothing.
 */
export function exampleLibrary(): LibraryRow[] {
  return [
    game({ title: "Disco Elysium: The Final Cut", cover_url: PORTRAIT_ART, owned_stores: ["steam", "gog"], playtime_minutes: 1240, last_played_at: 1_700_000_000, status: "finished", rating: 10, store_cover_url: WIDE_ART, summary: "A detective with no memory wakes in a city that is falling to pieces and must resolve a murder while he argues with himself. Each skill is a voice, and all of them lie a little.".repeat(2) }),
    game({ title: "Hades", cover_url: PORTRAIT_ART, playtime_minutes: 3120, last_played_at: 1_750_000_000, status: "playing", rating: 9, store_cover_url: WIDE_ART }),
    game({ title: "Ori and the Blind Forest: Definitive Edition", owned_stores: ["steam", "gog"], playtime_minutes: 660, status: "finished", rating: 8 }),
    game({ title: "Outer Wilds", playtime_minutes: 0, status: "backlog" }),
    game({ title: "Divinity: Original Sin 2", owned_stores: ["gog"], playtime_minutes: 0, status: "backlog" }),
    game({ title: "Cyberpunk 2077", owned_stores: ["gog"], playtime_minutes: 1860, last_played_at: 1_600_000_000, status: "abandoned", rating: 6 }),
    game({ title: "LIMBO", owned_stores: ["steam", "gog"], playtime_minutes: 240, status: "finished", rating: 7 }),
    game({ title: "Stardew Valley", owned_stores: ["steam", "gog"], playtime_minutes: 2400 }),
    game({ title: "A game that left every store", owned_stores: [], wishlist_stores: [], status: "playing" }),
  ];
}

/**
 * A review queue with the four shapes that a row has: the row that is equal —
 * with nothing selected — the row that wins clearly, the row that IGDB does not
 * know, and the row with a store title that does not fit in its column.
 */
export function exampleQueue(): ReviewItem[] {
  const candidate = (
    igdb_id: number,
    name: string,
    score: number,
    release_year: number | null,
  ) => ({ igdb_id, name, score, release_year, cover_url: null, slug: "record" });

  return [
    {
      store_entry_id: "11111111-1111-7111-8111-111111111111",
      store: "steam",
      title: "LIMBO",
      cover_url: WIDE_ART,
      store_url: "https://store.steampowered.com/app/48000",
      tie: true,
      candidates: [candidate(1, "Limbo", 1, 2010), candidate(2, "Limbo", 1, 2011)],
    },
    {
      store_entry_id: "22222222-2222-7222-8222-222222222222",
      store: "gog",
      title: "Ori and the Blind Forest: Definitive Edition",
      cover_url: null,
      store_url: null,
      tie: false,
      candidates: [
        candidate(3, "Ori and the Blind Forest: Definitive Edition", 0.97, 2016),
        candidate(4, "Ori and the Blind Forest", 0.81, 2015),
        candidate(5, "Ori and the Will of the Wisps", 0.55, 2020),
      ],
    },
    {
      store_entry_id: "33333333-3333-7333-8333-333333333333",
      store: "gog",
      title: "A game with a very long title that does not fit in its column",
      cover_url: null,
      store_url: null,
      tie: false,
      candidates: [],
    },
  ];
}

/**
 * A wishlist with the difficult conditions of that screen: a very long title, a
 * game that the user already has in a different store, a game at its all-time
 * low, a game that has never been discounted and a game that nobody sells.
 */
export function exampleWishlist(): { library: LibraryRow[]; prices: PriceRow[] } {
  const library = [
    game({ title: "Hollow Knight: Silksong", owned_stores: [], wishlist_stores: ["steam"] }),
    game({ title: "Blasphemous II", owned_stores: [], wishlist_stores: ["gog", "steam"] }),
    game({ title: "Baldur's Gate 3", owned_stores: ["gog"], wishlist_stores: ["steam"] }),
    game({
      title: "A wished-for game with a very long title that does not fit in its column",
      owned_stores: [],
      wishlist_stores: ["epic"],
    }),
    game({ title: "A game that nobody sells", owned_stores: [], wishlist_stores: ["gog"] }),
  ];

  const price = (row: LibraryRow, overrides: Partial<PriceRow>): PriceRow => ({
    game_id: row.game_id,
    shop: "GOG",
    amount: 1599,
    regular: 3999,
    cut: 60,
    currency: "EUR",
    shops: 4,
    low_all_time: 899,
    low_year: 1349,
    itad_slug: "a-game",
    captured_at: 1_755_000_000,
    ...overrides,
  });

  return {
    library,
    prices: [
      price(library[0]!, { amount: 899, cut: 75, shop: "Steam" }),
      price(library[1]!, { amount: 2399, cut: 40 }),
      price(library[2]!, { amount: 5999, regular: 5999, cut: 0, low_all_time: null, low_year: null, shops: 1 }),
      price(library[3]!, { amount: 199, cut: 95, shop: "GreenManGaming" }),
    ],
  };
}

function defaultAnswers(library: LibraryRow[]): Answers {
  return {
    app_info: { version: "0.1.0", secrets_backend: "keyring", unlocked: true },
    list_accounts: [
      { store: "steam", account_ref: "7656119", display_name: "serjor", last_sync_at: 1_755_000_000 },
    ],
    // No rows: no store switched off and no store with an error, which is the
    // usual condition and the condition to measure. The list of connectors with
    // a problem appears only when something occurs.
    connector_states: [],
    has_igdb_credentials: true,
    has_itad_credentials: true,
    library_summary: {
      owned: library.length,
      wishlist: library.filter((row) => row.wishlist_stores.length > 0).length,
      games: library.length,
      pending_review: 0,
    },
    review_queue: [],
    library,
    prices: [],
  };
}

/** A file server for `dist/`, on any free port. */
function serveDist() {
  return Bun.serve({
    port: 0,
    async fetch(request) {
      const path = new URL(request.url).pathname;
      const file = Bun.file(`dist${path === "/" ? "/index.html" : path}`);
      // A clean 404: the browser always asks for the favicon and it is not
      // there, and without this each page prints an exception that means
      // nothing.
      return (await file.exists()) ? new Response(file) : new Response(null, { status: 404 });
    },
  });
}

async function openBrowser(): Promise<Browser> {
  // No sandbox only as root, which is where the Chromium sandbox does not
  // start. On a usual desktop it stays on, which is what it is for.
  const args = process.getuid?.() === 0 ? ["--no-sandbox"] : [];
  const executablePath = process.env["CHROMIUM_PATH"];

  try {
    return await chromium.launch(executablePath ? { executablePath, args } : { args });
  } catch (cause) {
    throw new Error(
      "Could not find a Chromium to start. Install one with `bunx playwright " +
        "install chromium`, or say where it is with CHROMIUM_PATH.\n" +
        String(cause),
    );
  }
}

export interface Options {
  /** The width of the window. The height is almost never important. */
  width?: number;
  height?: number;
  theme?: "light" | "dark";
  /** What the bridge answers. What you do not give is filled in. */
  answers?: Partial<Answers>;
}

/**
 * Opens the application, gives it to you, and collects everything at the end.
 *
 *     await withTheApp(async (page) => {
 *       await page.getByRole("button", { name: "Covers" }).click();
 *       const overlap = await page.evaluate(() => { ... });
 *     });
 */
export async function withTheApp<T>(
  use: (page: Page) => Promise<T>,
  options: Options = {},
): Promise<T> {
  const server = serveDist();
  const browser = await openBrowser();
  const errors: string[] = [];

  try {
    const page = await browser.newPage({
      viewport: { width: options.width ?? 1200, height: options.height ?? 800 },
      colorScheme: options.theme ?? "light",
    });
    page.on("pageerror", (e) => errors.push(e.message));

    const answers: Answers = {
      ...defaultAnswers(options.answers?.library ?? exampleLibrary()),
      ...options.answers,
    };

    await page.addInitScript((data: Answers) => {
      let next = 0;
      const w = window as unknown as Record<string, unknown>;
      w["__TAURI_INTERNALS__"] = {
        invoke: (command: string) =>
          Promise.resolve(command in data ? data[command as keyof Answers] : null),
        // `listen` registers its callback here; without this the event bus
        // breaks at the start.
        transformCallback: (cb: unknown) => {
          next += 1;
          w[`_cb${next}`] = cb;
          return next;
        },
      };
    }, answers);

    await page.goto(`http://localhost:${server.port}/`);
    // The application does not render until it resolves all of the load
    // commands, thus to wait for the navigation is to wait for the seven of
    // them.
    await page.getByRole("navigation").waitFor();

    const result = await use(page);
    if (errors.length > 0) {
      throw new Error(`The page gave errors:\n${errors.join("\n")}`);
    }
    return result;
  } finally {
    await browser.close();
    await server.stop(true);
  }
}
