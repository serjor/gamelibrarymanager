/**
 * Las comprobaciones de maquetación que `bun test` no puede hacer.
 *
 * Cada una de las tres cazó un fallo de verdad mientras se escribía la interfaz,
 * y ninguna se veía a ojo en una captura:
 *
 * - Las portadas se pisaban unas a otras porque `aspect-ratio` sobre un item de
 *   rejilla no aporta altura a su fila.
 * - La cabecera de la tabla dejaba de cuadrar con sus columnas al desplazar.
 * - El rótulo de dos líneas se salía de su baldosa y se comía la fila de abajo.
 *
 *     bun run build && bun run visual
 */
import { conLaApp } from "./arnes";

let fallos = 0;

function comprobar(que: string, bien: boolean, detalle = "") {
  if (bien) {
    console.log(`  ok  ${que}`);
  } else {
    fallos += 1;
    console.log(`  NO  ${que}${detalle ? `\n      ${detalle}` : ""}`);
  }
}

/** De maximizada a lo más estrecho: el recorrido, no dos puntos sueltos. */
const ANCHOS = [1600, 1400, 1200, 1000, 900, 800, 700, 620];

console.log("\nPared de portadas");
for (const ancho of ANCHOS) {
  const r = await conLaApp(
    async (pagina) => {
      await pagina.getByRole("button", { name: "Portadas" }).click();
      await pagina.getByRole("checkbox").first().waitFor();
      return pagina.evaluate(() => {
        const cajas = [...document.querySelectorAll(".pared > li")].map((e) =>
          e.getBoundingClientRect(),
        );
        let solapan = false;
        for (let i = 0; i < cajas.length; i++) {
          for (let j = i + 1; j < cajas.length; j++) {
            const a = cajas[i]!;
            const b = cajas[j]!;
            if (
              a.left < b.right - 0.5 &&
              b.left < a.right - 0.5 &&
              a.top < b.bottom - 0.5 &&
              b.top < a.bottom - 0.5
            ) {
              solapan = true;
            }
          }
        }
        // Un rótulo largo que se sale de su baldosa descuadra todas las filas
        // de abajo, no solo la suya.
        const desbordan = [...document.querySelectorAll(".pared > li")].filter((li) => {
          const dentro = li.querySelector(".baldosa");
          return dentro !== null && dentro.getBoundingClientRect().height > li.getBoundingClientRect().height + 0.5;
        }).length;
        return {
          solapan,
          desbordan,
          deLado:
            document.documentElement.scrollWidth > document.documentElement.clientWidth,
        };
      });
    },
    { ancho },
  );

  comprobar(`${ancho} px · ninguna baldosa se solapa`, !r.solapan);
  comprobar(`${ancho} px · ningún rótulo se sale de su baldosa`, r.desbordan === 0);
  comprobar(`${ancho} px · la página no se va de lado`, !r.deLado);
}

console.log("\nTabla");
for (const ancho of ANCHOS) {
  const r = await conLaApp(
    async (pagina) => {
      await pagina.getByRole("columnheader").first().waitFor();
      return pagina.evaluate(() => {
        const izquierdas = (fila: Element) =>
          [...fila.children].map((c) => Math.round(c.getBoundingClientRect().left));
        const cabecera = document.querySelector("thead tr");
        const primera = document.querySelector("tbody tr:not([style])");
        const titulo = document.querySelector("tbody td.tt button");
        // La casilla no es texto, pero la celda sí recorta como si lo fuera:
        // cuando se salía por un píxel, el navegador pintaba unos puntos
        // suspensivos al lado de cada casilla de la tabla.
        const marca = document.querySelector("tbody tr:not([style]) td");
        return {
          marcaRecortada: marca !== null && marca.scrollWidth > marca.clientWidth + 1,
          cuadra:
            cabecera !== null &&
            primera !== null &&
            JSON.stringify(izquierdas(cabecera)) === JSON.stringify(izquierdas(primera)),
          tituloRecortado:
            titulo !== null && titulo.scrollWidth > titulo.clientWidth + 1,
          deLado:
            document.documentElement.scrollWidth > document.documentElement.clientWidth,
        };
      });
    },
    { ancho },
  );

  comprobar(`${ancho} px · la cabecera cuadra con las celdas`, r.cuadra);
  comprobar(`${ancho} px · el título no se recorta`, !r.tituloRecortado);
  comprobar(`${ancho} px · la casilla cabe en su celda`, !r.marcaRecortada);
  comprobar(`${ancho} px · la página no se va de lado`, !r.deLado);
}

/**
 * La ficha acoplada, que solo existe si la tabla le deja sitio al lado. El
 * número que decide —82rem en `Library.tsx`— es la suma de lo que necesita cada
 * pieza, y esto es lo que comprueba que la suma estaba bien hecha.
 */
console.log("\nLa ficha acoplada a la tabla");
for (const ancho of [1312, 1400, 1600]) {
  const r = await conLaApp(
    async (pagina) => {
      await pagina.locator("td.tt button").first().click();
      await pagina.locator(".detail").waitFor();
      return pagina.evaluate(() => {
        const inspector = document.querySelector(".detail")!.getBoundingClientRect();
        const caja = document.querySelector(".tabla-viewport")!;
        const tabla = caja.getBoundingClientRect();
        return {
          hoja: document.querySelector("dialog[open]") !== null,
          // La tabla se queda con lo que le deja el inspector: si no le llega,
          // se desplaza en horizontal y el título empieza a recortarse.
          tablaDeLado: caja.scrollWidth > caja.clientWidth + 1,
          pisa: inspector.left < tabla.right - 0.5,
          deLado: document.documentElement.scrollWidth > document.documentElement.clientWidth,
        };
      });
    },
    { ancho },
  );

  comprobar(`${ancho} px · la ficha se acopla, no se superpone`, !r.hoja);
  comprobar(`${ancho} px · la tabla entera cabe al lado del inspector`, !r.tablaDeLado);
  comprobar(`${ancho} px · el inspector no pisa la tabla`, !r.pisa);
  comprobar(`${ancho} px · la página no se va de lado`, !r.deLado);
}

console.log("\nLa ficha superpuesta");
// Con arte de tienda y sin él: la ficha sin nada que enseñar es la de quien no
// ha configurado IGDB, y es la que más fácil se abre con un agujero.
for (const [ancho, desde, juego, arte] of [
  [1200, "tabla", "Cyberpunk 2077", "DIV"],
  [1000, "tabla", "Disco Elysium: The Final Cut", "IMG"],
  [1400, "pared", "Disco Elysium: The Final Cut", "IMG"],
  [700, "pared", "Cyberpunk 2077", "DIV"],
] as const) {
  const r = await conLaApp(
    async (pagina) => {
      if (desde === "pared") {
        await pagina.getByRole("button", { name: "Portadas" }).click();
        // El nombre de la baldosa lleva detrás las tiendas y el estado.
        await pagina.getByRole("button", { name: new RegExp(`^${juego}`) }).click();
      } else {
        await pagina.getByRole("button", { name: juego, exact: true }).click();
      }
      await pagina.locator("dialog[open]").waitFor();
      return pagina.evaluate(() => {
        const caja = document.querySelector(".hoja-caja")!;
        const rect = caja.getBoundingClientRect();
        const banda = document.querySelector(".hoja-arte")!;
        const arte = banda.getBoundingClientRect();
        const velo = getComputedStyle(document.querySelector(".hoja")!).backgroundColor;
        return {
          inspector: document.querySelector(".detail") !== null,
          arte: banda.tagName,
          dentro:
            rect.top >= -0.5 &&
            rect.left >= -0.5 &&
            rect.bottom <= window.innerHeight + 0.5 &&
            rect.right <= window.innerWidth + 0.5,
          arteDesborda: arte.width > rect.width + 0.5,
          cuerpoDeLado: caja.scrollWidth > caja.clientWidth + 1,
          // El velo lo pinta el diálogo, no `::backdrop`: si el token no
          // llegara, esto saldría transparente y no se notaría a ojo.
          velo: velo !== "rgba(0, 0, 0, 0)" && velo !== "transparent",
        };
      });
    },
    { ancho },
  );

  comprobar(`${ancho} px desde ${desde} · se superpone, y no queda inspector`, !r.inspector);
  comprobar(`${ancho} px desde ${desde} · la hoja entera cabe en la ventana`, r.dentro);
  comprobar(
    `${ancho} px desde ${desde} · ${arte === "IMG" ? "el arte de la tienda" : "la banda de cuando no hay arte"}`,
    r.arte === arte,
  );
  comprobar(`${ancho} px desde ${desde} · el arte no se sale de la hoja`, !r.arteDesborda);
  comprobar(`${ancho} px desde ${desde} · la hoja no se desplaza en horizontal`, !r.cuerpoDeLado);
  comprobar(`${ancho} px desde ${desde} · el velo se pinta`, r.velo);
}

console.log("\nContraste del texto sobre su fondo");
for (const tema of ["light", "dark"] as const) {
  const r = await conLaApp(
    async (pagina) => {
      // Con la hoja abierta: es la única superficie que no se pinta sobre el
      // fondo de la página, y un atenuado que cumple sobre uno no cumple
      // automáticamente sobre el otro.
      await pagina.getByRole("button", { name: "Portadas" }).click();
      await pagina.locator(".baldosa").first().click();
      await pagina.locator("dialog[open]").waitFor();
      return pagina.evaluate(() => {
        const numeros = (s: string) => (s.match(/\d+/g) ?? []).map(Number);
        const luz = (c: number[]) => {
          const canal = (v: number) => {
            const x = v / 255;
            return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
          };
          return 0.2126 * canal(c[0] ?? 0) + 0.7152 * canal(c[1] ?? 0) + 0.0722 * canal(c[2] ?? 0);
        };
        const razon = (a: number[], b: number[]) => {
          const [alto, bajo] = [luz(a), luz(b)].sort((x, y) => y - x) as [number, number];
          return (alto + 0.05) / (bajo + 0.05);
        };
        const fondo = numeros(getComputedStyle(document.body).backgroundColor);
        const atenuado = document.querySelector(".hint");
        const hoja = document.querySelector(".hoja-caja")!;
        const fondoHoja = numeros(getComputedStyle(hoja).backgroundColor);
        const atenuadoHoja = hoja.querySelector(".hint");
        return {
          texto: razon(numeros(getComputedStyle(document.body).color), fondo),
          atenuado: atenuado
            ? razon(numeros(getComputedStyle(atenuado).color), fondo)
            : Number.NaN,
          enLaHoja: atenuadoHoja
            ? razon(numeros(getComputedStyle(atenuadoHoja).color), fondoHoja)
            : Number.NaN,
        };
      });
    },
    { tema },
  );

  // 4,5:1 es el mínimo de la AA para texto normal.
  comprobar(`${tema} · texto ${r.texto.toFixed(2)}:1`, r.texto >= 4.5);
  comprobar(`${tema} · atenuado ${r.atenuado.toFixed(2)}:1`, r.atenuado >= 4.5);
  comprobar(`${tema} · atenuado en la hoja ${r.enLaHoja.toFixed(2)}:1`, r.enLaHoja >= 4.5);
}

console.log(fallos === 0 ? "\nTodo cuadra.\n" : `\n${fallos} comprobaciones sin pasar.\n`);
process.exit(fallos === 0 ? 0 : 1);
