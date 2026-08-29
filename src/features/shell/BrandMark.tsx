/**
 * The product mark. The SVG only supplies geometry; styles.css supplies its
 * colours so the mark follows the active theme.
 */
export function BrandMark() {
  return (
    <div className="brand-mark" role="img" aria-label="Game Library Manager">
      <svg
        viewBox="0 0 64 64"
        aria-hidden="true"
        focusable="false"
      >
        <rect className="brand-mark__card--a" x="8" y="15" width="18" height="34" rx="4" />
        <rect className="brand-mark__card--b" x="17" y="10" width="19" height="38" rx="4" />
        <rect className="brand-mark__card--c" x="27" y="15" width="19" height="34" rx="4" />
        <rect className="brand-mark__frame" x="14" y="7" width="37" height="50" rx="7" />
      </svg>
    </div>
  );
}
