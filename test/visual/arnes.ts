/**
 * La aplicación de verdad, en un navegador, sin Tauri.
 *
 * Sirve `dist/` y abre Chromium con el puente de Tauri sustituido: `invoke` de
 * `@tauri-apps/api/core` llama a `window.__TAURI_INTERNALS__`, así que basta con
 * poner ahí un objeto con respuestas de mentira antes de que cargue el bundle.
 * No hace falta tocar ni una línea del proyecto ni dejar ningún mock dentro de
 * `src`.
 *
 * Para qué: para **medir** la interfaz, no para mirarla. Un `bun test` con
 * happy-dom no hace maquetación —mide todos los contenedores a cero—, así que
 * no puede decir si dos portadas se solapan, si una cabecera deja de cuadrar
 * con su columna o si un texto se sale de su caja. Eso solo lo sabe un motor de
 * maquetación de verdad, y mirarlo a ojo en una captura engaña: en la sesión
 * que escribió esto, tres «fallos» vistos en capturas no existían y uno que no
 * se veía sí.
 *
 * No lo ejecuta CI: necesita un Chromium, y descargarlo por cada `push` cuesta
 * más de lo que aporta. Es una herramienta de mano.
 *
 *     bun run build && bun run visual
 *
 * Chromium se busca por su cuenta. Si no lo encuentra:
 *
 *     bunx playwright install chromium      # o
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
 * Lo que contestaría Rust. Cada clave es el nombre de un comando de Tauri.
 *
 * Aquí tienen que estar **todos** los que la aplicación pide al arrancar. Lo
 * que falte se contesta con `null`, y un `null` donde el código espera una
 * lista revienta el primer pintado: lo que se ve entonces no es un error
 * legible sino que ninguna comprobación encuentra la pantalla y todas agotan
 * su espera de treinta segundos.
 */
export interface Respuestas {
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
 * Una cabecera apaisada con las proporciones de la de Steam (460×215), servida
 * desde la propia página: lo que se mide es la caja donde se recorta, y para
 * eso no hace falta el CDN ni haber configurado nada.
 */
export const ARTE_APAISADO =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    "<svg xmlns='http://www.w3.org/2000/svg' width='460' height='215'>" +
      "<rect width='460' height='215' fill='gray'/></svg>",
  );

export function juego(overrides: Partial<LibraryRow> = {}): LibraryRow {
  const title = overrides.title ?? "Juego";
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
 * Una biblioteca con los casos incómodos dentro: títulos que no caben en una
 * línea, juegos en dos tiendas, sin estrenar, sin estado y sin nota. Los casos
 * fáciles no rompen nada.
 */
export function bibliotecaDeEjemplo(): LibraryRow[] {
  return [
    juego({ title: "Disco Elysium: The Final Cut", owned_stores: ["steam", "gog"], playtime_minutes: 1240, last_played_at: 1_700_000_000, status: "finished", rating: 10, store_cover_url: ARTE_APAISADO, summary: "Un detective amnésico despierta en una ciudad que se cae a trozos y tiene que resolver un asesinato mientras discute consigo mismo. Cada habilidad es una voz, y todas mienten un poco.".repeat(2) }),
    juego({ title: "Hades", playtime_minutes: 3120, last_played_at: 1_750_000_000, status: "playing", rating: 9, store_cover_url: ARTE_APAISADO }),
    juego({ title: "Ori and the Blind Forest: Definitive Edition", owned_stores: ["steam", "gog"], playtime_minutes: 660, status: "finished", rating: 8 }),
    juego({ title: "Outer Wilds", playtime_minutes: 0, status: "backlog" }),
    juego({ title: "Divinity: Original Sin 2", owned_stores: ["gog"], playtime_minutes: 0, status: "backlog" }),
    juego({ title: "Cyberpunk 2077", owned_stores: ["gog"], playtime_minutes: 1860, last_played_at: 1_600_000_000, status: "abandoned", rating: 6 }),
    juego({ title: "LIMBO", owned_stores: ["steam", "gog"], playtime_minutes: 240, status: "finished", rating: 7 }),
    juego({ title: "Stardew Valley", owned_stores: ["steam", "gog"], playtime_minutes: 2400 }),
  ];
}

/**
 * Una cola de revisión con las cuatro formas que tiene una fila: la que empata
 * —sin nada elegido—, la que gana con holgura, la que IGDB no conoce, y la que
 * trae un título de tienda que no cabe en su columna.
 */
export function colaDeEjemplo(): ReviewItem[] {
  const candidato = (
    igdb_id: number,
    name: string,
    score: number,
    release_year: number | null,
  ) => ({ igdb_id, name, score, release_year, cover_url: null, slug: "ficha" });

  return [
    {
      store_entry_id: "11111111-1111-7111-8111-111111111111",
      store: "steam",
      title: "LIMBO",
      cover_url: ARTE_APAISADO,
      store_url: "https://store.steampowered.com/app/48000",
      tie: true,
      candidates: [candidato(1, "Limbo", 1, 2010), candidato(2, "Limbo", 1, 2011)],
    },
    {
      store_entry_id: "22222222-2222-7222-8222-222222222222",
      store: "gog",
      title: "Ori and the Blind Forest: Definitive Edition",
      cover_url: null,
      store_url: null,
      tie: false,
      candidates: [
        candidato(3, "Ori and the Blind Forest: Definitive Edition", 0.97, 2016),
        candidato(4, "Ori and the Blind Forest", 0.81, 2015),
        candidato(5, "Ori and the Will of the Wisps", 0.55, 2020),
      ],
    },
    {
      store_entry_id: "33333333-3333-7333-8333-333333333333",
      store: "gog",
      title: "Un juego con un título larguísimo que no cabe de ninguna manera en su columna",
      cover_url: null,
      store_url: null,
      tie: false,
      candidates: [],
    },
  ];
}

/**
 * Deseados con los casos incómodos de esa pantalla: un título larguísimo, un
 * juego que ya se tiene en otra tienda, uno en su mínimo histórico, uno que
 * nunca se rebajó y uno que no vende nadie.
 */
export function deseadosDeEjemplo(): { library: LibraryRow[]; prices: PriceRow[] } {
  const library = [
    juego({ title: "Hollow Knight: Silksong", owned_stores: [], wishlist_stores: ["steam"] }),
    juego({ title: "Blasphemous II", owned_stores: [], wishlist_stores: ["gog", "steam"] }),
    juego({ title: "Baldur's Gate 3", owned_stores: ["gog"], wishlist_stores: ["steam"] }),
    juego({
      title: "Un juego deseado con un título larguísimo que no cabe de ninguna manera en su columna",
      owned_stores: [],
      wishlist_stores: ["epic"],
    }),
    juego({ title: "Un juego que no vende nadie", owned_stores: [], wishlist_stores: ["gog"] }),
  ];

  const precio = (row: LibraryRow, overrides: Partial<PriceRow>): PriceRow => ({
    game_id: row.game_id,
    shop: "GOG",
    amount: 1599,
    regular: 3999,
    cut: 60,
    currency: "EUR",
    shops: 4,
    low_all_time: 899,
    low_year: 1349,
    itad_slug: "un-juego",
    captured_at: 1_755_000_000,
    ...overrides,
  });

  return {
    library,
    prices: [
      precio(library[0]!, { amount: 899, cut: 75, shop: "Steam" }),
      precio(library[1]!, { amount: 2399, cut: 40 }),
      precio(library[2]!, { amount: 5999, regular: 5999, cut: 0, low_all_time: null, low_year: null, shops: 1 }),
      precio(library[3]!, { amount: 199, cut: 95, shop: "GreenManGaming" }),
    ],
  };
}

function respuestasPorDefecto(library: LibraryRow[]): Respuestas {
  return {
    app_info: { version: "0.1.0", secrets_backend: "keyring", unlocked: true },
    list_accounts: [
      { store: "steam", account_ref: "7656119", display_name: "serjor", last_sync_at: 1_755_000_000 },
    ],
    // Sin filas: ninguna tienda apagada y ninguna con error, que es la
    // situación normal y la que hay que medir. La lista de conectores con
    // problema solo aparece cuando pasa algo.
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

/** Servidor de ficheros para `dist/`, en un puerto libre cualquiera. */
function servirDist() {
  return Bun.serve({
    port: 0,
    async fetch(peticion) {
      const ruta = new URL(peticion.url).pathname;
      const fichero = Bun.file(`dist${ruta === "/" ? "/index.html" : ruta}`);
      // Un 404 limpio: el navegador pide siempre el favicon y no está, y sin
      // esto cada página imprime una excepción que no significa nada.
      return (await fichero.exists()) ? new Response(fichero) : new Response(null, { status: 404 });
    },
  });
}

async function abrirNavegador(): Promise<Browser> {
  // Sin sandbox solo como root, que es donde el de Chromium no arranca. En un
  // escritorio normal se deja puesto, que para eso está.
  const args = process.getuid?.() === 0 ? ["--no-sandbox"] : [];
  const executablePath = process.env["CHROMIUM_PATH"];

  try {
    return await chromium.launch(executablePath ? { executablePath, args } : { args });
  } catch (causa) {
    throw new Error(
      "No he encontrado un Chromium que lanzar. Instálalo con `bunx playwright " +
        "install chromium`, o dime dónde está con CHROMIUM_PATH.\n" +
        String(causa),
    );
  }
}

export interface Opciones {
  /** Ancho de la ventana. El alto casi nunca importa. */
  ancho?: number;
  alto?: number;
  tema?: "light" | "dark";
  /** Lo que contesta el puente. Lo que no se diga, se rellena. */
  respuestas?: Partial<Respuestas>;
}

/**
 * Abre la aplicación, te la pasa, y recoge todo al terminar.
 *
 *     await conLaApp(async (pagina) => {
 *       await pagina.getByRole("button", { name: "Portadas" }).click();
 *       const solapan = await pagina.evaluate(() => { ... });
 *     });
 */
export async function conLaApp<T>(
  usar: (pagina: Page) => Promise<T>,
  opciones: Opciones = {},
): Promise<T> {
  const servidor = servirDist();
  const navegador = await abrirNavegador();
  const errores: string[] = [];

  try {
    const pagina = await navegador.newPage({
      viewport: { width: opciones.ancho ?? 1200, height: opciones.alto ?? 800 },
      colorScheme: opciones.tema ?? "light",
    });
    pagina.on("pageerror", (e) => errores.push(e.message));

    const respuestas: Respuestas = {
      ...respuestasPorDefecto(opciones.respuestas?.library ?? bibliotecaDeEjemplo()),
      ...opciones.respuestas,
    };

    await pagina.addInitScript((datos: Respuestas) => {
      let siguiente = 0;
      const w = window as unknown as Record<string, unknown>;
      w["__TAURI_INTERNALS__"] = {
        invoke: (comando: string) =>
          Promise.resolve(comando in datos ? datos[comando as keyof Respuestas] : null),
        // `listen` registra su callback por aquí; sin esto el bus de eventos
        // revienta al arrancar.
        transformCallback: (cb: unknown) => {
          siguiente += 1;
          w[`_cb${siguiente}`] = cb;
          return siguiente;
        },
      };
    }, respuestas);

    await pagina.goto(`http://localhost:${servidor.port}/`);
    // La aplicación no pinta hasta resolver todos los comandos de carga, así
    // que esperar a la navegación es esperar a que estén los siete.
    await pagina.getByRole("navigation").waitFor();

    const resultado = await usar(pagina);
    if (errores.length > 0) {
      throw new Error(`La página ha dado errores:\n${errores.join("\n")}`);
    }
    return resultado;
  } finally {
    await navegador.close();
    await servidor.stop(true);
  }
}
