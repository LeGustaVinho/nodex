import { useIsFetching } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useUi } from "../store";

/** A small spinning ring. */
export function Spinner({ size = 16 }: { size?: number }) {
  return <span className="spinner" style={{ width: size, height: size }} role="progressbar" aria-label="Loading" />;
}

/** Centered spinner with a message, filling its container (in place of a view that is loading). */
export function Loading({ label = "Loading…", detail }: { label?: string; detail?: string }) {
  return (
    <div className="loading" aria-busy="true">
      <Spinner size={28} />
      <div className="loading-label">{label}</div>
      {detail && <div className="muted small">{detail}</div>}
    </div>
  );
}

/** A dimming layer with a spinner over content that is being replaced (a new layout, a redraw). */
export function LoadingOverlay({ label, detail }: { label: string; detail?: string }) {
  return (
    <div className="loading-overlay" aria-busy="true">
      <div className="loading-card">
        <Spinner size={22} />
        <div>
          <div className="loading-label">{label}</div>
          {detail && <div className="muted small">{detail}</div>}
        </div>
      </div>
    </div>
  );
}

/** A small spinner badge for work that keeps the current content usable (a refresh). */
export function LoadingBadge({ label }: { label: string }) {
  return (
    <span className="loading-badge">
      <Spinner size={12} /> {label}
    </span>
  );
}

/**
 * A thin bar under the top bar while anything loads: a request, or a folder being opened.
 * It shows only after a short delay, so quick answers do not flicker it.
 */
export function ActivityBar() {
  const fetching = useIsFetching();
  const opening = useUi((s) => s.projectLoading !== null);
  const busy = fetching > 0 || opening;
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (!busy) {
      setShown(false);
      return;
    }
    const t = setTimeout(() => setShown(true), 150);
    return () => clearTimeout(t);
  }, [busy]);
  return <div className={`activity-bar${shown ? " on" : ""}`} aria-hidden={!shown} />;
}
