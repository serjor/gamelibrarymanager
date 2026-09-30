import { describe, expect, it } from "bun:test";
import { formatDuration, submissionsLabel } from "./timeToBeat";

describe("time to beat", () => {
  it("rounds to the nearest half hour", () => {
    expect(formatDuration(79_200)).toBe("22 h");
    expect(formatDuration(22 * 3600 + 20 * 60)).toBe("22½ h");
    expect(formatDuration(22 * 3600 + 50 * 60)).toBe("23 h");
  });

  it("keeps the minutes under one hour, and never says zero", () => {
    expect(formatDuration(45 * 60)).toBe("45 min");
    expect(formatDuration(10)).toBe("1 min");
  });

  it("shows a dash when the duration is unknown", () => {
    expect(formatDuration(null)).toBe("—");
    expect(formatDuration(0)).toBe("—");
  });

  it("says how many players the estimate comes from", () => {
    const time = { hastily: 1, normally: null, completely: null, submissions: 1 };
    expect(submissionsLabel(time)).toBe("Estimate from IGDB · 1 player");
    expect(submissionsLabel({ ...time, submissions: 1412 })).toBe(
      "Estimate from IGDB · 1,412 players",
    );
    expect(submissionsLabel({ ...time, submissions: 0 })).toBe("Estimate from IGDB");
  });
});
