import type { PlatformFamily } from "../../lib/api";
import { FAMILIES } from "./ManualWishForm";
import type { WishFilters } from "./view";

const PRICES: { value: NonNullable<WishFilters["price"]>; label: string }[] = [
  { value: "priced", label: "With a price" },
  { value: "discounted", label: "Discounted" },
  { value: "low", label: "At its all-time low" },
  { value: "none", label: "No price" },
];

/**
 * The filters of the wishlist. It uses the classes of the library filters, so
 * that the two bars look the same. A select that has nothing to choose does not
 * appear: with no manual wish there is no device to filter by, and with Luna
 * switched off no game has the mark.
 */
export function WishlistFilters({
  filters,
  stores,
  devices,
  luna,
  total,
  shown,
  onChange,
}: {
  filters: WishFilters;
  stores: string[];
  devices: PlatformFamily[];
  /** Whether Luna is switched on. With it off, no game has the mark. */
  luna: boolean;
  total: number;
  shown: number;
  onChange: (filters: WishFilters) => void;
}) {
  return (
    <div className="filters wishlist-filters" role="group" aria-label="Wishlist filters">
      <label className="filter-search">
        <span className="filter-label">Search</span>
        <input
          type="search"
          value={filters.search}
          onChange={(e) => onChange({ ...filters, search: e.target.value })}
          placeholder="Search in the wishlist"
          aria-label="Search in the wishlist"
        />
      </label>

      <div className="filter-set">
        {stores.length > 0 && (
          <label className="filter-field">
            <span className="filter-label">Store</span>
            <select
              value={filters.store ?? ""}
              onChange={(e) => onChange({ ...filters, store: e.target.value || null })}
            >
              <option value="">All of the stores</option>
              {stores.map((store) => (
                <option key={store} value={store}>
                  {store}
                </option>
              ))}
            </select>
          </label>
        )}

        {devices.length > 0 && (
          <label className="filter-field">
            <span className="filter-label">Device</span>
            <select
              value={filters.device ?? ""}
              onChange={(e) =>
                onChange({ ...filters, device: (e.target.value || null) as WishFilters["device"] })
              }
            >
              <option value="">Any device</option>
              {FAMILIES.filter((family) => devices.includes(family.value)).map((family) => (
                <option key={family.value} value={family.value}>
                  {family.label}
                </option>
              ))}
            </select>
          </label>
        )}

        <label className="filter-field">
          <span className="filter-label">Price</span>
          <select
            value={filters.price ?? ""}
            onChange={(e) =>
              onChange({ ...filters, price: (e.target.value || null) as WishFilters["price"] })
            }
          >
            <option value="">Any price</option>
            {PRICES.map((price) => (
              <option key={price.value} value={price.value}>
                {price.label}
              </option>
            ))}
          </select>
        </label>

        {luna && (
          <label className="filter-field">
            <span className="filter-label">Luna</span>
            <select
              value={filters.luna ?? ""}
              onChange={(e) =>
                onChange({ ...filters, luna: (e.target.value || null) as WishFilters["luna"] })
              }
            >
              <option value="">With or without Luna</option>
              <option value="on">On Luna</option>
              <option value="off">Not on Luna</option>
            </select>
          </label>
        )}
      </div>

      <output className="filter-count" aria-live="polite">
        {shown === total ? `${total} games` : `${shown} of ${total}`}
      </output>
    </div>
  );
}
