// The skins of other persons: the files of the skins folder, examined.
//
// Rust reads the files and examines nothing of the skin. Here each text goes
// through `validateSkin`, as an included skin does, and the result is the list
// for the selector and the list of problems that Utilities shows with the name
// of the file.
import type { UserSkinFile } from "../../lib/api";
import { validateSkin, type Skin, type SkinDefaults } from "./skin";

export interface UserSkin {
  skin: Skin;
  file: string;
  /** The text of the file, kept so that the start can apply it again with no wait. */
  text: string;
}

export interface SkinNotice {
  file: string;
  message: string;
}

export interface ExaminedSkins {
  skins: UserSkin[];
  /** The files that are not skins, and why. */
  refused: SkinNotice[];
  /** What a skin that is applied has and this version ignores. */
  warnings: SkinNotice[];
}

export function examineUserSkins(files: readonly UserSkinFile[], defaults: SkinDefaults): ExaminedSkins {
  const accepted: UserSkin[] = [];
  const refused: SkinNotice[] = [];
  const warnings: SkinNotice[] = [];

  for (const { file, text, error } of files) {
    if (text === undefined) {
      refused.push({ file, message: error ?? "it was not read" });
      continue;
    }
    const result = validateSkin(text, defaults);
    if (!result.ok) {
      refused.push({ file, message: result.error });
      continue;
    }
    accepted.push({ skin: result.skin, file, text });
    for (const message of result.warnings) warnings.push({ file, message });
  }

  // Two files with one id: no one can say which of the two the user chose.
  const count = new Map<string, number>();
  for (const { skin } of accepted) count.set(skin.id, (count.get(skin.id) ?? 0) + 1);
  const skins = accepted.filter(({ skin, file }) => {
    if (count.get(skin.id) === 1) return true;
    refused.push({ file, message: `the id "${skin.id}" is in more than one file` });
    return false;
  });

  return {
    skins,
    refused,
    warnings: warnings.filter((warning) => skins.some((user) => user.file === warning.file)),
  };
}
