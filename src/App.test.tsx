import { describe, expect, it, mock, beforeEach } from "bun:test";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import type {
  Account,
  AppInfo,
  LibraryRow,
  LibrarySummary,
  PlayStatus,
  ReviewItem,
} from "./lib/api";

const state = {
  info: { version: "0.1.0", secrets_backend: "keyring", unlocked: true } as AppInfo,
  accounts: [] as Account[],
  hasIgdb: true,
  summary: { owned: 0, wishlist: 0, games: 0, pending_review: 0 } as LibrarySummary,
  queue: [] as ReviewItem[],
  rows: [] as LibraryRow[],
  /** Lo que se ha escrito de verdad, para poder contarlo y mirarlo. */
  guardados: [] as [string, PlayStatus | null, number | null, string | null][],
  /** Los emparejamientos que ha llegado a confirmar el lote. */
  confirmados: [] as [string, number][],
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
    resolveIdentities: () =>
      Promise.resolve({ linked: 0, review: 0, unknown: 0, cancelled: false }),
    unlockSecrets: () => Promise.resolve(),
    connectSteam: () => Promise.resolve("id"),
    connectGog: () => Promise.resolve("id"),
    setIgdbCredentials: () => Promise.resolve(),
    reviewConfirm: () => Promise.resolve(),
    reviewConfirmMany: (decisions: [string, number][]) => {
      state.confirmados.push(...decisions);
      return Promise.resolve(decisions.length);
    },
    reviewWithoutMetadata: () => Promise.resolve(),
    library: () => Promise.resolve(state.rows),
    cancelOperation: () => Promise.resolve(),
    setUserState: (
      gameId: string,
      status: PlayStatus | null,
      rating: number | null,
      notes: string | null,
    ) => {
      state.guardados.push([gameId, status, rating, notes]);
      return Promise.resolve();
    },
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

function fila(overrides: Partial<LibraryRow>): LibraryRow {
  return {
    game_id: crypto.randomUUID(),
    title: "Juego",
    sort_title: "juego",
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
 * Cuatro filas y no más en los tests de tabla: sin layout de verdad, el
 * virtualizador mide el contenedor a cero y solo pinta su ventana de reserva.
 * Cuatro es lo que entra, y además es el tamaño de lote que pide el plan.
 */
const CUATRO = [
  fila({ title: "Celeste", sort_title: "celeste", playtime_minutes: 900, rating: 9, notes: "corto y redondo" }),
  fila({ title: "Hades", sort_title: "hades", playtime_minutes: 3120, rating: 8 }),
  fila({ title: "Outer Wilds", sort_title: "outer wilds", playtime_minutes: 0 }),
  fila({ title: "Prey", sort_title: "prey", playtime_minutes: 120, rating: 6 }),
];

/**
 * Una biblioteca con material para tres estanterías de «Hoy» a la vez: dos
 * empezados, uno sin estrenar y uno con copia en dos tiendas. Las fechas van
 * relativas al reloj porque los cortes de «Hoy» también.
 */
const AHORA = Math.floor(Date.now() / 1000);
const DIA = 86_400;
const PARA_HOY = [
  fila({ title: "Hades", sort_title: "hades", status: "playing", playtime_minutes: 3120, last_played_at: AHORA - 2 * DIA }),
  fila({ title: "Celeste", sort_title: "celeste", status: "playing", playtime_minutes: 900, last_played_at: AHORA - 100 * DIA }),
  fila({ title: "Prey", sort_title: "prey", owned_stores: ["steam", "gog"], playtime_minutes: 120, last_played_at: AHORA - 10 * DIA }),
  fila({ title: "Outer Wilds", sort_title: "outer wilds" }),
];

const marcaDe = (titulo: string) => screen.getByLabelText(`Seleccionar ${titulo}`) as HTMLInputElement;

/**
 * De qué juego es la ficha que hay abierta, sea acoplada o superpuesta.
 *
 * Por el identificador al que apunta `aria-labelledby`, y no por «el segundo
 * encabezado de la pantalla»: «Hoy» también tiene uno para su propuesta.
 */
const tituloDeLaFicha = () => document.getElementById("ficha-titulo");
const fichaAbierta = () => tituloDeLaFicha()?.textContent ?? null;

/**
 * Dentro de la ficha y no en toda la pantalla: el filtro de la barra también se
 * llama «Estado», y buscar por nombre a secas encuentra los dos.
 */
const enLaFicha = () => within(tituloDeLaFicha()!.closest(".ficha") as HTMLElement);

/** Dos fichas que puntúan igual: el motivo más común de acabar en la cola. */
const EMPATE: ReviewItem = {
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

/** Una entrada en la que un candidato gana con holgura: viene ya elegido. */
const HOLGADO: ReviewItem = {
  store_entry_id: "22222222-2222-7222-8222-222222222222",
  store: "gog",
  title: "Otro juego",
  cover_url: null,
  store_url: null,
  tie: false,
  candidates: [
    { igdb_id: 3, name: "Otro juego", score: 0.95, release_year: 2015, cover_url: null, slug: null },
    {
      igdb_id: 4,
      name: "Otro juego, pero de 2009",
      score: 0.72,
      release_year: 2009,
      cover_url: null,
      slug: null,
    },
  ],
};

/**
 * Lo que dice la columna «se emparejará con» de una entrada.
 *
 * Se busca por el título de la tienda dentro de su celda, y no a secas: el
 * candidato elegido puede llamarse igual que la entrada —de hecho es lo normal
 * cuando el emparejamiento acierta— y entonces el título sale dos veces.
 */
const emparejaCon = (titulo: string) =>
  screen.getByText(titulo, { selector: ".origen strong" }).closest("tr")?.cells[2]?.textContent;

/**
 * El ancho de la ventana es lo que decide entre inspector y hoja, así que aquí
 * hay que poder moverlo: happy-dom lo expone y `matchMedia` le hace caso.
 */
function anchura(px: number) {
  (
    window as unknown as { happyDOM: { setViewport: (v: { width: number }) => void } }
  ).happyDOM.setViewport({ width: px });
}

describe("App", () => {
  beforeEach(() => {
    // Ancha por defecto: es donde la biblioteca se ve entera, y la ventana
    // estrecha es lo que se prueba aparte.
    anchura(1400);
    state.info = { version: "0.1.0", secrets_backend: "keyring", unlocked: true };
    state.accounts = [];
    state.hasIgdb = true;
    state.summary = { owned: 0, wishlist: 0, games: 0, pending_review: 0 };
    state.queue = [];
    state.rows = [];
    state.guardados = [];
    state.confirmados = [];
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

  it("con las dos tiendas conectadas ya no ofrece conectar ninguna", async () => {
    state.accounts = [cuentaSteam, cuentaGog];
    render(<App />);
    await screen.findByRole("button", { name: "Sincronizar" });
    expect(screen.queryByRole("button", { name: "Conectar Steam" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Conectar GOG" })).toBeNull();
  });

  it("sin ninguna cuenta se puede empezar por GOG en vez de por Steam", async () => {
    render(<App />);
    (await screen.findByRole("button", { name: "o empezar por GOG" })).click();
    expect(await screen.findByRole("button", { name: /Iniciar sesión en GOG/ })).toBeDefined();
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
      fila({
        title: "Disco Elysium",
        sort_title: "disco elysium",
        release_year: 2019,
        genres: ["RPG"],
        owned_stores: ["steam", "gog"],
        playtime_minutes: 1240,
      }),
    ];
    render(<App />);
    // Cada juego es una fila, y en ella están las dos tiendas que lo tienen: es
    // lo que distingue una copia duplicada de una sola.
    const celda = await screen.findByRole("button", { name: "Disco Elysium" });
    const filaDom = celda.closest("tr");
    expect(filaDom?.textContent).toContain("steam");
    expect(filaDom?.textContent).toContain("gog");
    expect(filaDom?.textContent).toContain("21 h");
  });

  it("pulsar una columna ordena por ella, y volver a pulsarla da la vuelta", async () => {
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);

    const titulos = () =>
      screen.getAllByRole("button").filter((b) => b.className === "celda").map((b) => b.textContent);

    // De salida, por título.
    expect(await screen.findByRole("button", { name: "Celeste" })).toBeDefined();
    expect(titulos()).toEqual(["Celeste", "Hades", "Outer Wilds", "Prey"]);

    fireEvent.click(screen.getByRole("button", { name: /Horas/ }));
    // Ascendente, y lo que no se ha jugado al final aunque valga cero.
    expect(titulos()).toEqual(["Prey", "Celeste", "Hades", "Outer Wilds"]);

    fireEvent.click(screen.getByRole("button", { name: /Horas/ }));
    expect(titulos()).toEqual(["Hades", "Celeste", "Prey", "Outer Wilds"]);
  });

  it("con mayúsculas se selecciona el rango entero, no una fila", async () => {
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });

    fireEvent.click(marcaDe("Celeste"));
    expect(marcaDe("Celeste").checked).toBe(true);
    expect(marcaDe("Prey").checked).toBe(false);

    fireEvent.click(marcaDe("Prey"), { shiftKey: true });
    for (const titulo of ["Celeste", "Hades", "Outer Wilds", "Prey"]) {
      expect(marcaDe(titulo).checked).toBe(true);
    }
  });

  it("cambiar de vista no cambia qué juegos hay delante", async () => {
    // Filtro y orden se aplican en un solo sitio y las dos vistas pintan el
    // resultado. La comprobación es que no hay dos sitios donde divergir.
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });

    const conjunto = () =>
      screen
        .getAllByLabelText(/^Seleccionar /)
        .map((e) => e.getAttribute("aria-label"))
        .sort();

    fireEvent.change(screen.getByPlaceholderText("Buscar en la biblioteca"), {
      target: { value: "out" },
    });
    expect(conjunto()).toEqual(["Seleccionar Outer Wilds"]);

    fireEvent.click(screen.getByRole("button", { name: "Portadas" }));
    expect(conjunto()).toEqual(["Seleccionar Outer Wilds"]);
    // Y sigue siendo la pared, no la tabla disfrazada.
    expect(screen.queryByRole("columnheader")).toBeNull();
  });

  it("lo seleccionado en la tabla sigue seleccionado en las portadas", async () => {
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });

    fireEvent.click(marcaDe("Celeste"));
    fireEvent.click(marcaDe("Hades"), { shiftKey: true });

    fireEvent.click(screen.getByRole("button", { name: "Portadas" }));

    expect(marcaDe("Celeste").checked).toBe(true);
    expect(marcaDe("Hades").checked).toBe(true);
    expect(marcaDe("Prey").checked).toBe(false);
    // La barra de lote no se entera de que ha cambiado la vista.
    expect(screen.getByText("2 seleccionados")).toBeDefined();
  });

  it("el lote escribe una vez por juego y no se lleva por delante lo escrito", async () => {
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });

    fireEvent.click(marcaDe("Celeste"));
    fireEvent.click(marcaDe("Prey"), { shiftKey: true });

    fireEvent.change(screen.getByLabelText("Marcar como"), { target: { value: "abandoned" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));

    // Una llamada por juego, ni una más: el lote no puede escribir dos veces
    // sobre el mismo ni saltarse uno.
    await waitFor(() => expect(state.guardados).toHaveLength(4));
    expect(state.guardados.every(([, estado]) => estado === "abandoned")).toBe(true);
    // La nota y el texto se devuelven tal cual: `set_user_state` reescribe la
    // fila entera, y sin esto un cambio de estado en lote borraría en silencio
    // lo único que la aplicación sabe del usuario.
    const celeste = state.guardados.find(([id]) => id === CUATRO[0]!.game_id);
    expect(celeste?.[2]).toBe(9);
    expect(celeste?.[3]).toBe("corto y redondo");
  });

  it("desde la tabla la ficha se abre acoplada, y con ↑↓ se recorre la lista", async () => {
    // Es la razón de que el inspector exista: comparar juegos de uno en uno sin
    // volver a la tabla a buscar el siguiente ni perder la ficha al hacerlo.
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));

    expect(fichaAbierta()).toBe("Celeste");
    expect(screen.queryByRole("dialog")).toBeNull();

    fireEvent.keyDown(window, { key: "ArrowDown" });
    expect(fichaAbierta()).toBe("Hades");
    fireEvent.keyDown(window, { key: "ArrowUp" });
    expect(fichaAbierta()).toBe("Celeste");

    // Y en el extremo no se cierra ni salta al otro lado: se queda donde está.
    fireEvent.keyDown(window, { key: "ArrowUp" });
    expect(fichaAbierta()).toBe("Celeste");
  });

  it("escribiendo una nota, ↑↓ mueve el cursor y no de juego", async () => {
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));

    fireEvent.keyDown(enLaFicha().getByLabelText("Notas"), { key: "ArrowDown" });
    expect(fichaAbierta()).toBe("Celeste");
  });

  it("en una ventana estrecha la ficha de la tabla se abre en hoja", async () => {
    // Por debajo de su rango el inspector no cabe al lado de la tabla, y
    // dejarlo ahí recortaría el título justo cuando estás comparando fichas.
    anchura(1000);
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));

    expect(screen.getByRole("dialog")).toBeDefined();
    expect(fichaAbierta()).toBe("Celeste");
  });

  it("desde las portadas la ficha se abre en hoja, con el arte de la tienda", async () => {
    state.accounts = [cuentaSteam];
    state.rows = [
      fila({
        title: "Celeste",
        sort_title: "celeste",
        summary: "Ayuda a Madeline a sobrevivir a sus demonios.",
        store_cover_url: "https://cdn.cloudflare.steamstatic.com/steam/apps/504230/header.jpg",
      }),
    ];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Portadas" }));
    fireEvent.click(screen.getByRole("button", { name: /^Celeste/ }));

    expect(screen.getByRole("dialog")).toBeDefined();
    expect(screen.getByText(/sobrevivir a sus demonios/)).toBeDefined();
    // Decorativa —el título va debajo—, así que no tiene rol y se busca por lo
    // que es: la imagen apaisada de la tienda, que es lo que la hoja añade.
    const arte = document.querySelector(".hoja-arte");
    expect(arte?.tagName).toBe("IMG");
    expect(arte?.getAttribute("src")).toContain("header.jpg");
  });

  it("sin portada de tienda y sin resumen la hoja se abre igual", async () => {
    // El caso de quien no ha configurado IGDB, que es el que promete el aviso
    // de la cabecera: la ficha tiene menos que enseñar, no menos que funcionar.
    anchura(1000);
    state.accounts = [cuentaSteam];
    state.rows = [fila({ title: "Celeste", sort_title: "celeste" })];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));

    expect(screen.getByRole("dialog")).toBeDefined();
    expect(screen.getByText(/Sin resumen/)).toBeDefined();
    expect(document.querySelector(".hoja-arte")?.tagName).toBe("DIV");
    expect(enLaFicha().getByLabelText("Notas")).toBeDefined();
  });

  it("guardar desde la hoja escribe lo mismo que guardar desde el inspector", async () => {
    // Un solo formulario y un solo guardado: la presentación no puede cambiar
    // lo que llega a la base de datos.
    state.accounts = [cuentaSteam];
    state.rows = [fila({ title: "Celeste", sort_title: "celeste", rating: 9 })];

    for (const [ancho, esperado] of [
      [1400, null],
      [1000, "dialog"],
    ] as const) {
      anchura(ancho);
      const { unmount } = render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Celeste" }));
      expect(screen.queryByRole("dialog") === null ? null : "dialog").toBe(esperado);

      fireEvent.change(enLaFicha().getByLabelText("Estado"), { target: { value: "finished" } });
      fireEvent.click(enLaFicha().getByRole("button", { name: "Guardar" }));
      await waitFor(() => expect(state.guardados).toHaveLength(1));

      expect(state.guardados[0]).toEqual([state.rows[0]!.game_id, "finished", 9, null]);
      state.guardados = [];
      unmount();
    }
  });

  it("«Hoy» propone lo que tienes a medias, y no lo repite en las estanterías", async () => {
    // Proponer algo nuevo mientras tienes uno empezado es lo que hace crecer la
    // pila, que es justo lo que esta pantalla intenta deshacer.
    state.accounts = [cuentaSteam];
    state.rows = PARA_HOY;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Hoy" }));

    expect((await screen.findByRole("heading", { level: 2 })).textContent).toBe("Hades");
    expect(screen.getByText("Lo tienes a medias")).toBeDefined();

    // Cada estantería con su motivo, y solo las que tienen algo dentro: «hace
    // mucho que no lo tocas» no sale porque nada llega a los seis meses.
    expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual([
      "Lo dejaste a medias",
      "Sin estrenar",
      "Lo tienes dos veces",
    ]);

    // Y el destacado no vuelve a salir abajo: verlo dos veces en la misma
    // pantalla hace pensar que son dos juegos.
    expect(screen.queryAllByRole("button", { name: /^Hades/ })).toHaveLength(0);
    expect(screen.getByRole("button", { name: /^Celeste/ })).toBeDefined();
  });

  it("«Hoy» no enseña estanterías vacías ni revienta con la biblioteca vacía", async () => {
    state.accounts = [cuentaSteam];
    state.rows = [];
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Hoy" }));

    expect(await screen.findByText(/Todavía no hay ningún juego en propiedad/)).toBeDefined();
    expect(screen.queryByRole("heading", { level: 3 })).toBeNull();
  });

  it("«Hoy» hace sus propios cortes y no hereda los filtros de la biblioteca", async () => {
    // Es lo que separa «Hoy» de un tercer modo de vista: la tabla y la pared
    // comparten contrato —filtras y las dos enseñan lo filtrado—, y esta
    // pantalla propone lo suyo.
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);
    await screen.findByRole("button", { name: "Celeste" });

    fireEvent.change(screen.getByPlaceholderText("Buscar en la biblioteca"), {
      target: { value: "celeste" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Hoy" }));

    expect(screen.getByRole("heading", { level: 2 }).textContent).toBe("Outer Wilds");
  });

  it("desde «Hoy» la ficha se abre en hoja aunque la ventana sea ancha", async () => {
    // Aquí no hay una lista al lado que mantener a la vista, y lo que se está
    // mirando es el arte.
    state.accounts = [cuentaSteam];
    state.rows = CUATRO;
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Hoy" }));
    fireEvent.click(screen.getByRole("button", { name: "Abrir la ficha" }));

    expect(screen.getByRole("dialog")).toBeDefined();
    expect(fichaAbierta()).toBe("Outer Wilds");
  });

  it("lo que gana con holgura viene ya elegido, y lo que empata no", async () => {
    // La asimetría es la cola entera: repetir con un clic lo que la pantalla ya
    // dice es trabajo inventado, y elegir por el usuario en un empate es
    // exactamente lo que el umbral se negó a hacer.
    state.accounts = [cuentaSteam];
    state.queue = [EMPATE, HOLGADO];
    render(<App />);
    (await screen.findByRole("button", { name: /Por revisar \(2\)/ })).click();

    await screen.findByText("Empates (1)");
    expect(emparejaCon("LIMBO")).toBe("sin elegir");
    expect(emparejaCon("Otro juego")).toContain("Otro juego");
    // Y el lote solo se lleva el que viene puesto.
    expect(screen.getByRole("button", { name: /Confirmar 1 emparejamiento$/ })).toBeDefined();
  });

  it("el lote escribe lo que enseña la columna, no lo que se tocó", async () => {
    state.accounts = [cuentaSteam];
    state.queue = [HOLGADO];
    render(<App />);
    (await screen.findByRole("button", { name: /Por revisar \(1\)/ })).click();

    // Se cambia por la otra ficha: la elegida y la descartada se intercambian.
    fireEvent.click(await screen.findByRole("button", { name: /^Otro juego, pero de 2009/ }));
    expect(emparejaCon("Otro juego")).toContain("Otro juego, pero de 2009");

    fireEvent.click(screen.getByRole("button", { name: /Confirmar 1 emparejamiento$/ }));
    await waitFor(() => expect(state.confirmados).toHaveLength(1));
    expect(state.confirmados[0]).toEqual([HOLGADO.store_entry_id, 4]);
  });

  it("quitar la ficha que venía puesta deja la entrada fuera del lote", async () => {
    // Es la única forma de decir «esta no» sin decir cuál sí, y hace falta para
    // dejar una entrada sin resolver mientras se confirman las demás.
    state.accounts = [cuentaSteam];
    state.queue = [HOLGADO];
    render(<App />);
    (await screen.findByRole("button", { name: /Por revisar \(1\)/ })).click();

    // El elegido va sin año ni parecido —los dos tienen su columna—, así que
    // su nombre accesible es el título a secas y no se confunde con el otro.
    fireEvent.click(await screen.findByRole("button", { name: "Otro juego" }));
    expect(emparejaCon("Otro juego")).toBe("sin elegir");
    expect(screen.queryByRole("button", { name: /Confirmar/ })).toBeNull();
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
    state.queue = [EMPATE, HOLGADO];
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
    state.queue = [EMPATE];
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
