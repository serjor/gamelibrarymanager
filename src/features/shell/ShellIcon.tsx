/**
 * Small local icons for the product navigation.
 *
 * The SVGs contain geometry only. The stylesheet supplies the colours so the
 * icons follow the active theme and do not become a second palette.
 */
export type ShellIconName = "library" | "today" | "wishlist" | "review" | "utilities";

export function ShellIcon({ name }: { name: ShellIconName }) {
  return (
    <svg
      className={`shell-icon shell-icon--${name}`}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      {name === "library" && (
        <>
          <rect className="shell-icon__stroke" x="4" y="5" width="12" height="15" rx="2" />
          <path className="shell-icon__stroke" d="M8 3h9a2 2 0 0 1 2 2v12" />
          <path className="shell-icon__stroke" d="M8 9h5M8 13h5M8 17h3" />
        </>
      )}
      {name === "today" && (
        <>
          <circle className="shell-icon__stroke" cx="12" cy="12" r="4" />
          <path className="shell-icon__stroke" d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M19.1 4.9l-1.4 1.4M6.3 17.7l-1.4 1.4" />
        </>
      )}
      {name === "wishlist" && (
        <path
          className="shell-icon__stroke"
          d="M12 20.2S4 15.7 4 9.7A4.2 4.2 0 0 1 12 7a4.2 4.2 0 0 1 8 2.7c0 6-8 10.5-8 10.5Z"
        />
      )}
      {name === "review" && (
        <>
          <path className="shell-icon__stroke" d="M5 5.5A2.5 2.5 0 0 1 7.5 3h9A2.5 2.5 0 0 1 19 5.5v8a2.5 2.5 0 0 1-2.5 2.5h-5l-3.8 3v-3H7.5A2.5 2.5 0 0 1 5 13.5Z" />
          <path className="shell-icon__stroke" d="m8.5 9.5 2.1 2.1 4.9-5" />
        </>
      )}
      {name === "utilities" && (
        <>
          <path className="shell-icon__stroke" d="M5 6h14M5 12h14M5 18h14" />
          <circle className="shell-icon__fill" cx="9" cy="6" r="2" />
          <circle className="shell-icon__fill" cx="15" cy="12" r="2" />
          <circle className="shell-icon__fill" cx="10" cy="18" r="2" />
        </>
      )}
    </svg>
  );
}
