import type { PlayStatus } from "./api";

/**
 * The status labels, in one place.
 *
 * The game record wrote them alone, and now the table and the bulk bar write
 * them too. Three copies of the same list is the easiest way to let "Backlog"
 * have two names one day, one name for each place that you look at.
 */
export const STATUS_LABEL: Record<PlayStatus, string> = {
  backlog: "Backlog",
  playing: "Playing",
  finished: "Finished",
  abandoned: "Abandoned",
};

/** In the order in which you go through a game, which is the sort order by status. */
export const STATUSES: PlayStatus[] = ["backlog", "playing", "finished", "abandoned"];
