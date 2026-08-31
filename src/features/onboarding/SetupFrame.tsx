import type { ReactNode } from "react";
import { BrandMark } from "../shell/BrandMark";

interface SetupFrameProps {
  step: string;
  title: string;
  description: string;
  trust: string;
  children: ReactNode;
  onBack?: () => void;
}

/**
 * The shared first-use surface.
 *
 * The forms keep their own fields, validation and API calls. This frame only
 * gives them the same context: what step the user is on, why the application
 * can be trusted, and where Back goes when the form came from Utilities.
 */
export function SetupFrame({
  step,
  title,
  description,
  trust,
  children,
  onBack,
}: SetupFrameProps) {
  return (
    <section className="setup-frame" aria-labelledby="setup-frame-title">
      <div className="setup-context">
        <div className="setup-brand">
          <BrandMark />
          <div className="setup-brand-copy">
            <strong>Game Library Manager</strong>
            <span>Personal archive</span>
          </div>
        </div>

        <p className="setup-step">{step}</p>
        <h1 id="setup-frame-title">{title}</h1>
        <p className="setup-description">{description}</p>

        <aside className="setup-trust" aria-label="Privacy information">
          <span className="setup-trust-mark" aria-hidden="true">
            ✓
          </span>
          <div>
            <strong>Private by design</strong>
            <p>{trust}</p>
          </div>
        </aside>
      </div>

      <div className="setup-form-surface">
        {children}
        {onBack && (
          <button type="button" className="link setup-back" onClick={onBack}>
            Back
          </button>
        )}
      </div>
    </section>
  );
}

export function SetupLoading({
  error,
  onRetry,
}: {
  error: string | null;
  onRetry: () => void;
}) {
  return (
    <SetupFrame
      step="Starting · Local archive"
      title="Opening your library"
      description="The application is preparing your local collection. Your data stays on this computer."
      trust="The application reads the local database first and does not need a network connection to show saved records."
    >
      <div className="setup-loading" aria-busy={error === null}>
        <span className="setup-loading-mark" aria-hidden="true" />
        {error === null ? (
          <p className="setup-loading-message" role="status" aria-live="polite">
            Opening the library…
          </p>
        ) : (
          <p className="setup-loading-message" role="alert">
            {error}
          </p>
        )}
      </div>
      {error !== null && (
        <button type="button" className="primary-action" onClick={onRetry}>
          Try again
        </button>
      )}
    </SetupFrame>
  );
}
