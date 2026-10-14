// The skins that the application includes.
//
// They come from the ten candidates of the landing page in commit 4007daf
// (site/NN-*/index.html). None of those pages has two modes: the mode of each
// page is translated to the roles of the tokens, and the other mode is
// designed to match it. A test examines each file with `validateSkin`, the
// same examination that a skin from another person gets.
import { SKIN_FORMAT, type Skin } from "../features/shell/skin";
import arcade from "./arcade.json";
import cards from "./cards.json";
import demoscene from "./demoscene.json";
import handheld from "./handheld.json";
import jrpg from "./jrpg.json";
import magazine from "./magazine.json";
import memoryCard from "./memory-card.json";
import pointAndClick from "./point-and-click.json";
import souls from "./souls.json";
import speedrun from "./speedrun.json";

type SkinFile = Omit<Skin, "format"> & { format: number };

// `$schema` serves the editor of the file and is not part of the skin.
function included(file: SkinFile & { $schema?: string }): Skin {
  const { $schema, ...skin } = file;
  void $schema;
  return { ...skin, format: SKIN_FORMAT };
}

/** In the order of the landing page. */
export const INCLUDED_SKINS: readonly Skin[] = [
  arcade,
  jrpg,
  magazine,
  speedrun,
  demoscene,
  memoryCard,
  handheld,
  cards,
  pointAndClick,
  souls,
].map(included);

