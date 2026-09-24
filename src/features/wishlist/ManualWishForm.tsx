import { useState, type FormEvent } from "react";
import {
  api,
  errorMessage,
  type IgdbCandidate,
  type LibraryRow,
  type PlatformFamily,
  type WishTarget,
} from "../../lib/api";

const FAMILIES: { value: PlatformFamily; label: string }[] = [
  { value: "pc", label: "PC" },
  { value: "playstation", label: "PlayStation" },
  { value: "xbox", label: "Xbox" },
  { value: "nintendo", label: "Nintendo" },
  { value: "other", label: "Other" },
];

export function deviceLabel(family: PlatformFamily, model: string): string {
  return model || FAMILIES.find((item) => item.value === family)?.label || family;
}

export function ManualWishForm({
  rows,
  hasIgdb,
  onAdd,
  onClose,
}: {
  rows: LibraryRow[];
  hasIgdb: boolean;
  onAdd: (target: WishTarget, family: PlatformFamily, model: string) => Promise<void>;
  onClose: () => void;
}) {
  const [source, setSource] = useState<"existing" | "igdb" | "title">(
    rows.length > 0 ? "existing" : "title",
  );
  const [query, setQuery] = useState("");
  const [selectedGame, setSelectedGame] = useState("");
  const [selectedIgdb, setSelectedIgdb] = useState<number | null>(null);
  const [candidates, setCandidates] = useState<IgdbCandidate[]>([]);
  const [family, setFamily] = useState<PlatformFamily>("pc");
  const [model, setModel] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const matches = rows
    .filter((row) => row.title.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()))
    .slice(0, 30);

  const search = async () => {
    setBusy(true);
    setError(null);
    setSelectedIgdb(null);
    setCandidates([]);
    try {
      setCandidates(await api.searchManualWishGames(query.trim()));
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const target: WishTarget | null = source === "existing"
      ? selectedGame ? { kind: "existing", game_id: selectedGame } : null
      : source === "igdb"
        ? selectedIgdb !== null ? { kind: "igdb", igdb_id: selectedIgdb } : null
        : query.trim() ? { kind: "title", title: query.trim() } : null;
    if (!target) {
      setError("Choose a game or enter a title.");
      return;
    }
    if (family === "other" && !model.trim()) {
      setError("Enter a model for Other.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await onAdd(target, family, model.trim());
      onClose();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="manual-wish-form" onSubmit={(event) => void submit(event)}>
      <h2>Add a wanted game</h2>
      <p className="hint">Choose a record to keep its notes, find a game in IGDB, or enter a new title.</p>
      <fieldset>
        <legend>Game source</legend>
        {rows.length > 0 && <label><input type="radio" name="wish-source" checked={source === "existing"} onChange={() => setSource("existing")} /> In my library</label>}
        {hasIgdb && <label><input type="radio" name="wish-source" checked={source === "igdb"} onChange={() => setSource("igdb")} /> Search IGDB</label>}
        <label><input type="radio" name="wish-source" checked={source === "title"} onChange={() => setSource("title")} /> New title</label>
      </fieldset>

      {source === "existing" && <>
        <label htmlFor="wish-game-query">Find a library game</label>
        <input id="wish-game-query" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search by title" />
        <label htmlFor="wish-existing-game">Game</label>
        <select id="wish-existing-game" value={selectedGame} onChange={(event) => setSelectedGame(event.target.value)} required>
          <option value="">Choose a game</option>
          {matches.map((row) => <option key={row.game_id} value={row.game_id}>{row.title}</option>)}
        </select>
      </>}

      {source === "igdb" && <>
        <label htmlFor="wish-igdb-query">Search title</label>
        <div className="manual-wish-search">
          <input id="wish-igdb-query" value={query} onChange={(event) => { setQuery(event.target.value); setCandidates([]); setSelectedIgdb(null); }} />
          <button type="button" disabled={busy || !query.trim()} onClick={() => void search()}>Search</button>
        </div>
        {candidates.length > 0 && <fieldset className="manual-wish-results"><legend>IGDB results</legend>
          {candidates.map((candidate) => <label key={candidate.igdb_id}>
            <input type="radio" name="igdb-result" checked={selectedIgdb === candidate.igdb_id} onChange={() => setSelectedIgdb(candidate.igdb_id)} />
            {candidate.name}{candidate.release_year && ` (${candidate.release_year})`}
          </label>)}
        </fieldset>}
        {candidates.length === 0 && <p className="hint">Search IGDB, or choose New title if the game is absent.</p>}
      </>}

      {source === "title" && <>
        <label htmlFor="wish-new-title">Game title</label>
        <input id="wish-new-title" value={query} onChange={(event) => setQuery(event.target.value)} maxLength={200} required />
      </>}

      <label htmlFor="wish-family">Device family</label>
      <select id="wish-family" value={family} onChange={(event) => setFamily(event.target.value as PlatformFamily)}>
        {FAMILIES.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
      </select>
      <label htmlFor="wish-model">Model {family === "other" ? "(required)" : "(optional)"}</label>
      <input id="wish-model" value={model} onChange={(event) => setModel(event.target.value)} maxLength={80} placeholder={family === "playstation" ? "PS5" : family === "nintendo" ? "Switch" : family === "xbox" ? "Series X" : family === "pc" ? "PC" : "Device model"} required={family === "other"} />
      {error && <p role="alert">{error}</p>}
      <div className="manual-wish-buttons">
        <button type="submit" disabled={busy}> {busy ? "Saving…" : "Add to wishlist"}</button>
        <button type="button" className="link" onClick={onClose}>Cancel</button>
      </div>
    </form>
  );
}

export { FAMILIES };
