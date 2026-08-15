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
        return {
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
  comprobar(`${ancho} px · la página no se va de lado`, !r.deLado);
}

console.log("\nContraste del texto sobre su fondo");
for (const tema of ["light", "dark"] as const) {
  const r = await conLaApp(
    (pagina) =>
      pagina.evaluate(() => {
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
        return {
          texto: razon(numeros(getComputedStyle(document.body).color), fondo),
          atenuado: atenuado
            ? razon(numeros(getComputedStyle(atenuado).color), fondo)
            : Number.NaN,
        };
      }),
    { tema },
  );

  // 4,5:1 es el mínimo de la AA para texto normal.
  comprobar(`${tema} · texto ${r.texto.toFixed(2)}:1`, r.texto >= 4.5);
  comprobar(`${tema} · atenuado ${r.atenuado.toFixed(2)}:1`, r.atenuado >= 4.5);
}

console.log(fallos === 0 ? "\nTodo cuadra.\n" : `\n${fallos} comprobaciones sin pasar.\n`);
process.exit(fallos === 0 ? 0 : 1);
