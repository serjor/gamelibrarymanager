import { useState } from "react";
import { api, errorMessage, type ReviewItem } from "../../lib/api";

/**
 * Lo que el emparejamiento automático no se atrevió a decidir.
 *
 * Que esta cola exista es la decisión de diseño central del producto: un
 * duplicado visible molesta, pero dos juegos distintos fusionados le hacen
 * perder al usuario el estado y las notas de uno de los dos, y encima sin
 * avisar. Ante la duda, se pregunta.
 */
export function ReviewQueue({ items, onResolved }: { items: ReviewItem[]; onResolved: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const act = async (id: string, action: () => Promise<void>) => {
    setBusy(id);
    setError(null);
    try {
      await action();
      onResolved();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  if (items.length === 0) {
    return <p className="hint">No hay nada pendiente de revisar.</p>;
  }

  return (
    <section>
      <h2>Por revisar ({items.length})</h2>
      {error && <p role="alert">{error}</p>}
      <ul className="review">
        {items.map((item) => (
          <li key={item.store_entry_id}>
            <p>
              <strong>{item.title}</strong> <span className="hint">· {item.store}</span>
            </p>
            {item.candidates.length === 0 ? (
              <p className="hint">IGDB no conoce este juego.</p>
            ) : (
              <ul className="candidates">
                {item.candidates.map((candidate) => (
                  <li key={candidate.igdb_id}>
                    <button
                      disabled={busy === item.store_entry_id}
                      onClick={() =>
                        void act(item.store_entry_id, () =>
                          api.reviewConfirm(item.store_entry_id, candidate.igdb_id),
                        )
                      }
                    >
                      {candidate.name}
                      <span className="hint"> · {Math.round(candidate.score * 100)}%</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <button
              className="link"
              disabled={busy === item.store_entry_id}
              onClick={() =>
                void act(item.store_entry_id, () =>
                  api.reviewWithoutMetadata(item.store_entry_id),
                )
              }
            >
              Ninguno: crear ficha con el título de la tienda
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
