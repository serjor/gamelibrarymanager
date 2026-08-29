/**
 * The layout tests that `bun test` cannot make.
 *
 * Each of the three found a real defect while the interface was written, and you
 * could see none of them in a screenshot:
 *
 * - The covers covered each other because `aspect-ratio` on a grid item gives no
 *   height to its row.
 * - The header of the table stopped agreeing with its columns while it scrolled.
 * - The label of two lines went out of its tile and covered the row below.
 *
 *     bun run build && bun run visual
 */
import { LONG_PROVIDER_ERROR, exampleLibrary, exampleQueue, exampleWishlist, openUtilities, withTheApp } from "./harness";

/**
 * An application with material in the four screens at the same time: owned games
 * for the library and "Today", wished-for games with a price, and a review
 * queue. Without this, an empty screen passes the tests with no render.
 */
const WISHES = exampleWishlist();
const ALL = {
  library: [...exampleLibrary(), ...WISHES.library],
  prices: WISHES.prices,
  review_queue: exampleQueue(),
};

let failures = 0;

function check(what: string, ok: boolean, detail = "") {
  if (ok) {
    console.log(`  ok  ${what}`);
  } else {
    failures += 1;
    console.log(`  NO  ${what}${detail ? `\n      ${detail}` : ""}`);
  }
}

/** From the largest to the narrowest: the complete path, not two points. */
const WIDTHS = [1920, 1600, 1400, 1200, 1000, 900, 800, 700, 620];

/**
 * The shell keeps its four destinations labelled at every width. At a wide
 * width the navigation is a rail; below the breakpoint it becomes top navigation.
 * The shell itself remains fixed while each feature owns its content scroll.
 */
console.log("\nThe application shell");
for (const width of [1400, 1000, 620]) {
  const r = await withTheApp(
    async (page) => {
      await page.getByRole("navigation").waitFor();
      return page.evaluate(() => {
        const shell = document.querySelector(".app-shell")!;
        const rail = document.querySelector(".shell-rail")!;
        const content = document.querySelector(".shell-content")!;
        const navigation = document.querySelector(".shell-navigation")!;
        const list = document.querySelector(".shell-navigation-list")!;
        const railStyle = getComputedStyle(rail);
        const navigationStyle = getComputedStyle(navigation);
        const listStyle = getComputedStyle(list);
        const labels = [...document.querySelectorAll(".shell-nav-item")]
          .map((item) => item.getAttribute("aria-label") ?? "")
          .join("|");
        const railBox = rail.getBoundingClientRect();
        const contentBox = content.getBoundingClientRect();
        return {
          labels,
          wide: railStyle.display === "flex" && listStyle.flexDirection === "column",
          compact:
            railStyle.display === "grid" &&
            listStyle.flexDirection === "row" &&
            navigationStyle.overflowX === "auto",
          railBeforeContent: railBox.right <= contentBox.left + 1,
          shellScrolls: shell.scrollHeight > shell.clientHeight + 1,
          pageSideways: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
        };
      });
    },
    { width, height: 900, answers: ALL },
  );

  const where = width + " px";
  check(where + " · navigation labels every workspace", r.labels === "Library|Today|Wishlist (5)|Review (3)");
  check(where + " · shell does not scroll", !r.shellScrolls);
  check(where + " · page does not go sideways", !r.pageSideways);
  if (width >= 1120) {
    check(where + " · wide rail is vertical", r.wide && r.railBeforeContent);
  } else {
    check(where + " · compact navigation is horizontal", r.compact);
  }
}

console.log("\nThe utility dialog");
const utilityState = await withTheApp(
  async (page) => {
    const trigger = page.getByRole("button", { name: "Utilities" });
    await trigger.focus();
    await openUtilities(page);
    const containment = await page.locator("dialog[open]").evaluate((dialog) => {
      const rect = dialog.getBoundingClientRect();
      return {
        inside: rect.top >= -0.5 && rect.left >= -0.5 &&
          rect.bottom <= window.innerHeight + 0.5 && rect.right <= window.innerWidth + 0.5,
        hasContent: dialog.querySelector(".utility-content") !== null,
      };
    });
    await page.getByRole("button", { name: "Close Utilities" }).click();
    await page.waitForFunction(() => document.querySelector("dialog[open]") === null);
    const focusReturned = await page.evaluate(() =>
      document.activeElement?.getAttribute("aria-controls") === "utility-dialog",
    );
    return { ...containment, focusReturned };
  },
  { width: 620, height: 900, answers: ALL },
);
check("utility dialog stays inside the window", utilityState.inside);
check("utility dialog has a content region", utilityState.hasContent);
check("closing utilities returns focus to its trigger", utilityState.focusReturned);

console.log("\nOne scroll bar");
for (const width of [1920, 1400, 1000]) {
  for (const screenName of ["Library", "Today", "Wishlist", "Review"] as const) {
    const r = await withTheApp(
      async (page) => {
        if (screenName !== "Library") {
          await page.getByRole("button", { name: new RegExp(`^${screenName}`) }).click();
        }
        await page.getByRole("navigation").waitFor();
        return page.evaluate(() => {
          const root = document.documentElement;
          const frame = document.querySelector(".shell-workspace")!;
          // What really scrolls: the list, "Today", the wishlist or the queue.
          const region = document.querySelector(
            ".table-viewport, .wall-viewport, .today, .wishlist, .review-screen",
          )!;
          const box = region.getBoundingClientRect();
          const workspace = frame.getBoundingClientRect();
          return {
            pageScrolls: root.scrollHeight > root.clientHeight + 1,
            frameScrolls: frame.scrollHeight > frame.clientHeight + 1,
            // Edge to edge, less the padding of `main`, which is 24 px.
            edgeToEdge:
              box.left <= workspace.left + 25 && box.right >= workspace.right - 25,
            // And that real space stays: a region one hundred pixels high would
            // obey all of the above and would be useless.
            height: Math.round(box.height),
          };
        });
      },
      { width, height: 900, answers: ALL },
    );

    const where = `${width} px · ${screenName}`;
    check(`${where} · the page does not scroll`, !r.pageScrolls);
    check(`${where} · and the frame does not: only the region scrolls`, !r.frameScrolls);
    check(`${where} · the region reaches the two edges`, r.edgeToEdge);
    check(`${where} · and it keeps the height that stays (${r.height} px)`, r.height > 400);
  }
}

/**
 * A broken store cannot take the space of the library.
 *
 * The message of a connector with a problem goes in the header, above the region
 * that scrolls, and the store writes the message: it can be long and in a narrow
 * window it uses several lines. Everything that grows there takes space from the
 * list, thus the test uses the longest message that the connector can produce.
 */
console.log("\nA store with a problem in the header");
const LONG_MESSAGE = LONG_PROVIDER_ERROR;

for (const width of [1400, 620]) {
  const r = await withTheApp(
    async (page) => {
      await openUtilities(page);
      await page.getByRole("button", { name: "Switch Epic off" }).waitFor();
      return page.evaluate(() => {
        const root = document.documentElement;
        const region = document.querySelector(".table-viewport")!;
        const aviso = document.querySelector(".activity-problems")!.getBoundingClientRect();
        return {
          pageScrolls: root.scrollHeight > root.clientHeight + 1,
          goesSideways: root.scrollWidth > root.clientWidth + 1,
          height: Math.round(region.getBoundingClientRect().height),
          messageInside: aviso.right <= window.innerWidth + 0.5,
        };
      });
    },
    {
      width,
      height: 900,
      answers: {
        connector_states: [{ store: "epic", enabled: true, last_error: LONG_MESSAGE }],
      },
    },
  );

  check(`${width} px · the page still does not scroll`, !r.pageScrolls);
  check(`${width} px · the message does not go sideways`, !r.goesSideways && r.messageInside);
  check(`${width} px · the list keeps space (${r.height} px)`, r.height > 400);
}

console.log("\nThe wall of covers");
for (const width of WIDTHS) {
  const r = await withTheApp(
    async (page) => {
      await page.getByRole("button", { name: "Covers" }).click();
      await page.getByRole("checkbox").first().waitFor();
      return page.evaluate(() => {
        const boxes = [...document.querySelectorAll(".wall > li")].map((e) =>
          e.getBoundingClientRect(),
        );
        let overlap = false;
        for (let i = 0; i < boxes.length; i++) {
          for (let j = i + 1; j < boxes.length; j++) {
            const a = boxes[i]!;
            const b = boxes[j]!;
            if (
              a.left < b.right - 0.5 &&
              b.left < a.right - 0.5 &&
              a.top < b.bottom - 0.5 &&
              b.top < a.bottom - 0.5
            ) {
              overlap = true;
            }
          }
        }
        // A long label that goes out of its tile moves all of the rows below,
        // not only its own row.
        const overflow = [...document.querySelectorAll(".wall > li")].filter((li) => {
          const inner = li.querySelector(".tile");
          return inner !== null && inner.getBoundingClientRect().height > li.getBoundingClientRect().height + 0.5;
        }).length;
        const artwork = [...document.querySelectorAll(".wall > li .game-artwork")];
        const artworkSized = artwork.length > 0 && artwork.every((item) => {
          const box = item.getBoundingClientRect();
          return Math.abs(box.width - 150) < 0.5 && Math.abs(box.height - 200) < 0.5;
        });
        const fallback = artwork.filter((item) => item.classList.contains("game-artwork--fallback"));
        return {
          overlap,
          overflow,
          artworkSized,
          hasRealArtwork: artwork.some((item) => item.classList.contains("game-artwork--image")),
          hasFallbackMonogram: fallback.length > 0 && fallback.every((item) => (item.textContent ?? "").trim().length > 0),
          sideways:
            document.documentElement.scrollWidth > document.documentElement.clientWidth,
        };
      });
    },
    { width },
  );

  check(`${width} px · no tile covers another`, !r.overlap);
  check(`${width} px · no label goes out of its tile`, r.overflow === 0);
  check(`${width} px · artwork uses the 150 by 200 box`, r.artworkSized);
  check(`${width} px · real and fallback artwork are present`, r.hasRealArtwork && r.hasFallbackMonogram);
  check(`${width} px · the page does not go sideways`, !r.sideways);
}

console.log("\nThe table");
for (const width of WIDTHS) {
  const r = await withTheApp(
    async (page) => {
      await page.getByRole("columnheader").first().waitFor();
      return page.evaluate(() => {
        const lefts = (row: Element) =>
          [...row.children].map((c) => Math.round(c.getBoundingClientRect().left));
        const header = document.querySelector("thead tr");
        const first = document.querySelector("tbody tr:not([style])");
        const title = document.querySelector("tbody td.tt button");
        // The check box is not text, but the cell cuts as if it were: when it
        // was one pixel too large, the browser drew an ellipsis beside each
        // check box of the table.
        const check_ = document.querySelector("tbody tr:not([style]) td");
        return {
          checkCut: check_ !== null && check_.scrollWidth > check_.clientWidth + 1,
          aligned:
            header !== null &&
            first !== null &&
            JSON.stringify(lefts(header)) === JSON.stringify(lefts(first)),
          titleCut:
            title !== null && title.scrollWidth > title.clientWidth + 1,
          sideways:
            document.documentElement.scrollWidth > document.documentElement.clientWidth,
        };
      });
    },
    { width },
  );

  check(`${width} px · the header aligns with the cells`, r.aligned);
  check(`${width} px · the title is not cut`, !r.titleCut);
  check(`${width} px · the check box fits in its cell`, !r.checkCut);
  check(`${width} px · the page does not go sideways`, !r.sideways);
}

/**
 * The record beside the table, which exists only if the table leaves space for
 * it. The number that decides — 96rem in `Library.tsx` — is the sum of what each
 * piece needs, and this is what examines whether the sum was correct.
 */
console.log("\nThe record beside the table");
for (const width of [1535, 1536, 1600]) {
  const r = await withTheApp(
    async (page) => {
      await page.locator("td.tt button").first().click();
      await page.locator(".detail, dialog[open]").first().waitFor();
      return page.evaluate(() => {
        const inspector = document.querySelector(".detail");
        const box = document.querySelector(".table-viewport")!;
        const table = box.getBoundingClientRect();
        return {
          sheet: document.querySelector("dialog[open]") !== null,
          // The table keeps the space that the inspector leaves: if that is not
          // sufficient, it scrolls horizontally and the title starts to be cut.
          tableSideways: box.scrollWidth > box.clientWidth + 1,
          covers: inspector !== null && inspector.getBoundingClientRect().left < table.right - 0.5,
          sideways: document.documentElement.scrollWidth > document.documentElement.clientWidth,
        };
      });
    },
    { width },
  );

  check(`${width} px · the record presentation is correct`, width >= 1536 ? !r.sheet : r.sheet);
  check(`${width} px · all of the table fits beside the inspector`, width < 1600 || !r.tableSideways);
  check(`${width} px · the inspector does not cover the table`, width < 1536 || !r.covers);
  check(`${width} px · the page does not go sideways`, !r.sideways);
}

console.log("\nThe record on top of the covers");
// With store art and with none: the record with nothing to show is the record of
// a user who has not configured IGDB, and it is the record that most easily
// opens with a hole.
for (const [width, from, gameName, art] of [
  [1200, "table", "Cyberpunk 2077", "SPAN"],
  [1000, "table", "Disco Elysium: The Final Cut", "IMG"],
  [1400, "wall", "Disco Elysium: The Final Cut", "IMG"],
  [700, "wall", "Cyberpunk 2077", "SPAN"],
] as const) {
  const r = await withTheApp(
    async (page) => {
      if (from === "wall") {
        await page.getByRole("button", { name: "Covers" }).click();
        // The name of the tile carries the stores and the status after it.
        await page.getByRole("button", { name: new RegExp(`^${gameName}`) }).click();
      } else {
        await page.getByRole("button", { name: gameName, exact: true }).click();
      }
      await page.locator("dialog[open]").waitFor();
      return page.evaluate(() => {
        const box = document.querySelector(".sheet-box")!;
        const rect = box.getBoundingClientRect();
        const band = document.querySelector(".sheet-art")!;
        const artBox = band.getBoundingClientRect();
        const scrim = getComputedStyle(document.querySelector(".sheet")!).backgroundColor;
        return {
          inspector: document.querySelector(".detail") !== null,
          art: band.tagName,
          inside:
            rect.top >= -0.5 &&
            rect.left >= -0.5 &&
            rect.bottom <= window.innerHeight + 0.5 &&
            rect.right <= window.innerWidth + 0.5,
          artOverflows: artBox.width > rect.width + 0.5,
          bodySideways: box.scrollWidth > box.clientWidth + 1,
          // The dialog draws the scrim, not `::backdrop`: if the token did not
          // arrive, this would be transparent and you would not see it.
          scrim: scrim !== "rgba(0, 0, 0, 0)" && scrim !== "transparent",
        };
      });
    },
    { width },
  );

  check(`${width} px from ${from} · it covers, and no inspector stays`, !r.inspector);
  check(`${width} px from ${from} · all of the sheet fits in the window`, r.inside);
  check(
    `${width} px from ${from} · ${art === "IMG" ? "the store art" : "the band for when there is no art"}`,
    r.art === art,
  );
  check(`${width} px from ${from} · the art does not go out of the sheet`, !r.artOverflows);
  check(`${width} px from ${from} · the sheet does not scroll horizontally`, !r.bodySideways);
  check(`${width} px from ${from} · the scrim is drawn`, r.scrim);
}

console.log("\nToday");
for (const width of WIDTHS) {
  const r = await withTheApp(
    async (page) => {
      await page.getByRole("button", { name: "Today" }).click();
      await page.locator(".featured").waitFor();
      return page.evaluate(() => {
        const box = document.querySelector(".featured")!;
        const rect = box.getBoundingClientRect();
        const featuredArt = box.querySelector(".featured-art .game-artwork");
        const shelfArtwork = [...document.querySelectorAll(".shelf .game-artwork")];
        const shelfArtworkSized = shelfArtwork.length > 0 && shelfArtwork.every((item) => {
          const artworkBox = item.getBoundingClientRect();
          return Math.abs(artworkBox.width - 150) < 0.5 && Math.abs(artworkBox.height - 200) < 0.5;
        });
        return {
          hasWideBackdrop: box.querySelector(".featured-backdrop") !== null,
          hasPortraitAnchor: featuredArt?.classList.contains("game-artwork--image") ?? false,
          shelfArtworkSized,
          // It is the only piece of this screen with two columns, thus it is
          // the only piece that can have no space for the text.
          featuredOverflows: [...box.querySelectorAll("*")].some(
            (inner) => inner.getBoundingClientRect().right > rect.right + 0.5,
          ),
          // The tile is the same tile as the wall tile, with the same sizes: if
          // it goes out of its slot here, the shelf does not obey them.
          tilesOutside: [...document.querySelectorAll(".shelf > li")].filter((slot) => {
            const inner = slot.querySelector(".tile");
            return (
              inner !== null &&
              inner.getBoundingClientRect().height > slot.getBoundingClientRect().height + 0.5
            );
          }).length,
          shelves: document.querySelectorAll(".shelf").length,
          sideways: document.documentElement.scrollWidth > document.documentElement.clientWidth,
        };
      });
    },
    { width },
  );

  check(`${width} px · the featured game does not go out of its box`, !r.featuredOverflows);
  check(`${width} px · the featured game has wide and portrait artwork`, r.hasWideBackdrop && r.hasPortraitAnchor);
  check(`${width} px · shelf artwork uses the 150 by 200 box`, r.shelfArtworkSized);
  check(`${width} px · no tile goes out of its slot`, r.tilesOutside === 0);
  check(`${width} px · there are shelves to show`, r.shelves > 0);
  check(`${width} px · the page does not go sideways`, !r.sideways);
}

/**
 * The wishlist. The five columns of numbers have a fixed width and in them goes
 * formatted money, which has a different size in each currency, plus a label
 * below: what does not fit in its cell goes on top of the next cell.
 */
console.log("\nThe wishlist");
for (const width of WIDTHS) {
  const r = await withTheApp(
    async (page) => {
      await page.getByRole("button", { name: /^Wishlist/ }).click();
      await page.locator(".wishlist-table tbody tr").first().waitFor();
      return page.evaluate(() => {
        const lefts = (row: Element) =>
          [...row.children].map((c) => Math.round(c.getBoundingClientRect().left));
        const header = document.querySelector(".wishlist-table thead tr");
        const first = document.querySelector(".wishlist-table tbody tr");
        const overflow = [...document.querySelectorAll(".wishlist-table td")].filter((cell) => {
          const box = cell.getBoundingClientRect();
          return [...cell.querySelectorAll("*")].some(
            (inner) => inner.getBoundingClientRect().right > box.right + 0.5,
          );
        }).length;
        return {
          aligned:
            header !== null &&
            first !== null &&
            JSON.stringify(lefts(header)) === JSON.stringify(lefts(first)),
          overflow,
          // The largest discount at the top: it is the order that makes the
          // screen useful, and the test looks at the DOM shown and not at the
          // function that sorts it.
          firstTitle: document.querySelector(".wish-title")?.textContent ?? "",
          sideways: document.documentElement.scrollWidth > document.documentElement.clientWidth,
        };
      });
    },
    { width, answers: ALL },
  );

  check(`${width} px · the header aligns with the cells`, r.aligned);
  check(`${width} px · no cell goes out of its column`, r.overflow === 0);
  check(
    `${width} px · the discount controls (${r.firstTitle})`,
    r.firstTitle.startsWith("A wished-for game with a very long title"),
  );
  check(`${width} px · the page does not go sideways`, !r.sideways);
}

console.log("\nThe review queue");
for (const width of WIDTHS) {
  const r = await withTheApp(
    async (page) => {
      await page.getByRole("button", { name: /Review/ }).click();
      await page.locator(".review tbody tr").first().waitFor();
      return page.evaluate(() => {
        const lefts = (row: Element) =>
          [...row.children].map((c) => Math.round(c.getBoundingClientRect().left));
        const header = document.querySelector(".review thead tr");
        const first = document.querySelector(".review tbody tr");
        // The columns have a fixed width and in them goes everything: covers,
        // cards and store titles with no spaces. What goes out of its cell goes
        // on top of the next cell.
        const overflow = [...document.querySelectorAll(".review td")].filter((cell) => {
          const box = cell.getBoundingClientRect();
          return [...cell.querySelectorAll("*")].some(
            (inner) => inner.getBoundingClientRect().right > box.right + 0.5,
          );
        }).length;
        // And in the cell, each candidate must fit in its slot. The list makes
        // them the same height, thus what goes out at the bottom pushes nothing:
        // it is drawn on top of what comes after. The test looks at the slot and
        // not at the cell because that is where it occurs — the IGDB link went
        // out of its card and landed on the "none" button, and it never went out
        // of the cell.
        const goOutside = [...document.querySelectorAll(".review .candidates > li")].filter(
          (slot) => {
            const box = slot.getBoundingClientRect();
            return [...slot.querySelectorAll("*")].some(
              (inner) => inner.getBoundingClientRect().bottom > box.bottom + 0.5,
            );
          },
        ).length;
        return {
          aligned:
            header !== null &&
            first !== null &&
            JSON.stringify(lefts(header)) === JSON.stringify(lefts(first)),
          overflow,
          goOutside,
          sideways: document.documentElement.scrollWidth > document.documentElement.clientWidth,
        };
      });
    },
    { width, answers: { review_queue: exampleQueue() } },
  );

  check(`${width} px · the header aligns with the cells`, r.aligned);
  check(`${width} px · no cell goes out of its column`, r.overflow === 0);
  check(`${width} px · no candidate goes out of its slot`, r.goOutside === 0);
  check(`${width} px · the page does not go sideways`, !r.sideways);
}

/**
 * The only colour of the wishlist screen, measured in the two themes. A green
 * that reads well on the light background becomes weak on the dark background,
 * and that is exactly where it marks the data that users most look for.
 */
console.log("\nThe contrast of \"at its low\"");
for (const theme of ["light", "dark"] as const) {
  const ratio = await withTheApp(
    async (page) => {
      await page.getByRole("button", { name: /^Wishlist/ }).click();
      await page.locator(".low").first().waitFor();
      return page.evaluate(() => {
        const numbers = (s: string) => (s.match(/\d+/g) ?? []).map(Number);
        const light = (c: number[]) => {
          const channel = (v: number) => {
            const x = v / 255;
            return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
          };
          return 0.2126 * channel(c[0] ?? 0) + 0.7152 * channel(c[1] ?? 0) + 0.0722 * channel(c[2] ?? 0);
        };
        const mark = document.querySelector(".low")!;
        const [high, low] = [
          light(numbers(getComputedStyle(mark).color)),
          light(numbers(getComputedStyle(document.body).backgroundColor)),
        ].sort((a, b) => b - a) as [number, number];
        return (high + 0.05) / (low + 0.05);
      });
    },
    { theme, answers: ALL },
  );

  check(`${theme} · "at its low" ${ratio.toFixed(2)}:1`, ratio >= 4.5);
}

console.log("\nThe contrast of the text on its background");
for (const theme of ["light", "dark"] as const) {
  const r = await withTheApp(
    async (page) => {
      // With the sheet open: it is the only surface that is not drawn on the
      // background of the page, and a muted colour that obeys the rule on one
      // does not automatically obey it on the other.
      await page.getByRole("button", { name: "Covers" }).click();
      await page.locator(".tile").first().click();
      await page.locator("dialog[open]").waitFor();
      return page.evaluate(() => {
        const numbers = (s: string) => (s.match(/\d+/g) ?? []).map(Number);
        const light = (c: number[]) => {
          const channel = (v: number) => {
            const x = v / 255;
            return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
          };
          return 0.2126 * channel(c[0] ?? 0) + 0.7152 * channel(c[1] ?? 0) + 0.0722 * channel(c[2] ?? 0);
        };
        const ratio = (a: number[], b: number[]) => {
          const [high, low] = [light(a), light(b)].sort((x, y) => y - x) as [number, number];
          return (high + 0.05) / (low + 0.05);
        };
        const background = numbers(getComputedStyle(document.body).backgroundColor);
        const muted = document.querySelector(".hint");
        const sheet = document.querySelector(".sheet-box")!;
        const sheetBackground = numbers(getComputedStyle(sheet).backgroundColor);
        const sheetMuted = sheet.querySelector(".hint");
        return {
          text: ratio(numbers(getComputedStyle(document.body).color), background),
          muted: muted
            ? ratio(numbers(getComputedStyle(muted).color), background)
            : Number.NaN,
          inTheSheet: sheetMuted
            ? ratio(numbers(getComputedStyle(sheetMuted).color), sheetBackground)
            : Number.NaN,
        };
      });
    },
    { theme },
  );

  // 4.5:1 is the AA minimum for usual text.
  check(`${theme} · text ${r.text.toFixed(2)}:1`, r.text >= 4.5);
  check(`${theme} · muted ${r.muted.toFixed(2)}:1`, r.muted >= 4.5);
  check(`${theme} · muted in the sheet ${r.inTheSheet.toFixed(2)}:1`, r.inTheSheet >= 4.5);
}

console.log("\nThe contrast of focus and the primary action");
for (const theme of ["light", "dark"] as const) {
  const r = await withTheApp(
    async (page) => {
      await openUtilities(page);
      const primary = page.locator("button.primary-action");
      await primary.waitFor();
      // Opening Utilities used a pointer click. Restore keyboard modality so
      // the programmatic focus below matches the :focus-visible rule.
      await page.keyboard.press("Tab");
      await primary.focus();
      return page.evaluate(() => {
        const numbers = (s: string) => (s.match(/[0-9]+/g) ?? []).map(Number);
        const light = (c: number[]) => {
          const channel = (v: number) => {
            const x = v / 255;
            return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
          };
          return 0.2126 * channel(c[0] ?? 0) + 0.7152 * channel(c[1] ?? 0) + 0.0722 * channel(c[2] ?? 0);
        };
        const ratio = (a: number[], b: number[]) => {
          const [high, low] = [light(a), light(b)].sort((x, y) => y - x) as [number, number];
          return (high + 0.05) / (low + 0.05);
        };
        const primary = document.querySelector("button.primary-action")!;
        const styles = getComputedStyle(primary);
        return {
          action: ratio(numbers(styles.color), numbers(styles.backgroundColor)),
          focus: ratio(numbers(styles.outlineColor), numbers(getComputedStyle(document.body).backgroundColor)),
          visible: styles.outlineStyle !== "none" && styles.outlineWidth !== "0px",
        };
      });
    },
    { theme, answers: ALL },
  );

  check(theme + " · primary-action text " + r.action.toFixed(2) + ":1", r.action >= 4.5);
  check(theme + " · focus " + r.focus.toFixed(2) + ":1", r.focus >= 3);
  check(theme + " · focus indicator is visible", r.visible);
}

console.log(failures === 0 ? "\nAll correct.\n" : `\n${failures} tests did not pass.\n`);
process.exit(failures === 0 ? 0 : 1);
