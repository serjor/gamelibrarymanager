import { useState } from "react";
import type { ReactNode } from "react";
import type { LibrarySummary } from "../../lib/api";
import { BrandMark } from "./BrandMark";
import { ShellIcon, type ShellIconName } from "./ShellIcon";
import { UtilityPanel, type UtilityPanelProps } from "./UtilityPanel";

export type AppTab = "library" | "today" | "wishlist" | "review";

interface NavigationItem {
  tab: AppTab;
  label: string;
  icon: ShellIconName;
}

const NAVIGATION: NavigationItem[] = [
  { tab: "library", label: "Library", icon: "library" },
  { tab: "today", label: "Today", icon: "today" },
  { tab: "wishlist", label: "Wishlist", icon: "wishlist" },
  { tab: "review", label: "Review", icon: "review" },
];

function pageTitle(tab: AppTab): string {
  return NAVIGATION.find((item) => item.tab === tab)?.label ?? "Library";
}

function PrimaryNavigation({
  tab,
  wishlistCount,
  reviewCount,
  onTabChange,
}: {
  tab: AppTab;
  wishlistCount: number;
  reviewCount: number;
  onTabChange: (tab: AppTab) => void;
}) {
  return (
    <nav className="shell-navigation" aria-label="Primary navigation">
      <ul className="shell-navigation-list">
        {NAVIGATION.map((item) => {
          const count = item.tab === "wishlist" ? wishlistCount : item.tab === "review" ? reviewCount : 0;
          const active = tab === item.tab;
          return (
            <li key={item.tab}>
              <button
                type="button"
                className={active ? "shell-nav-item active" : "shell-nav-item"}
                aria-label={count > 0 ? `${item.label} (${count})` : item.label}
                aria-current={active ? "page" : undefined}
                onClick={() => onTabChange(item.tab)}
              >
                <ShellIcon name={item.icon} />
                <span>{item.label}</span>
                {count > 0 && <span className="shell-nav-count">{count}</span>}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}

function UtilityTrigger({
  open,
  onOpen,
}: {
  open: boolean;
  onOpen: () => void;
}) {
  return (
    <button
      type="button"
      className="shell-utility-trigger"
      aria-haspopup="dialog"
      aria-expanded={open}
      aria-controls="utility-dialog"
      onClick={onOpen}
    >
      <ShellIcon name="utilities" />
      <span>Utilities</span>
    </button>
  );
}

export interface AppShellProps {
  tab: AppTab;
  onTabChange: (tab: AppTab) => void;
  wishlistCount: number;
  reviewCount: number;
  summary: LibrarySummary | null;
  activity: ReactNode;
  utility: Omit<UtilityPanelProps, "open" | "onClose">;
  children: ReactNode;
}

/**
 * The product shell.
 *
 * At a wide width the same labeled navigation is a rail. At a smaller width it
 * becomes a compact top navigation. The workspace is the only page-level flex
 * region; its feature owns the scrollable content inside it.
 */
export function AppShell({
  tab,
  onTabChange,
  wishlistCount,
  reviewCount,
  summary,
  activity,
  utility,
  children,
}: AppShellProps) {
  const [utilitiesOpen, setUtilitiesOpen] = useState(false);

  return (
    <div className="app-shell">
      <aside className="shell-rail">
        <div className="shell-brand">
          <BrandMark />
          <div className="shell-brand-copy">
            <strong>Game library</strong>
            <span>Personal archive</span>
          </div>
        </div>

        <PrimaryNavigation
          tab={tab}
          wishlistCount={wishlistCount}
          reviewCount={reviewCount}
          onTabChange={onTabChange}
        />

        <UtilityTrigger open={utilitiesOpen} onOpen={() => setUtilitiesOpen(true)} />
      </aside>

      <div className="shell-content">
        {activity && <div className="shell-activity-slot">{activity}</div>}
        <main className="shell-workspace">
          <header className="shell-page-header">
            <div>
              <p className="shell-kicker">Collection workspace</p>
              <h1 id="workspace-title">{pageTitle(tab)}</h1>
            </div>
            {summary !== null && (
              <p className="summary workspace-summary">
                {summary.games} records · {summary.owned} owned copies · {summary.wishlist} wished for
                {summary.pending_review > 0 && ` · ${summary.pending_review} to review`}
              </p>
            )}
          </header>
          {children}
        </main>
      </div>

      <UtilityPanel
        {...utility}
        open={utilitiesOpen}
        onClose={() => setUtilitiesOpen(false)}
      />
    </div>
  );
}
