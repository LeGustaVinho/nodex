import { useUi } from "../store";

/** Files found, then files read, while a folder opens (the server reports about fifty steps). */
export function OpenProgressLine() {
  const progress = useUi((s) => s.projectProgress);
  if (!progress) return null;
  const reading = progress.phase === "reading" && progress.total > 0;
  const text = reading
    ? `Reading files ${progress.done.toLocaleString()} / ${progress.total.toLocaleString()}`
    : `Finding files… ${progress.done.toLocaleString()}`;
  return (
    <div className="open-progress">
      <span className="muted small">{text}</span>
      {reading && (
        <div className="open-progress-bar" role="progressbar" aria-valuemin={0} aria-valuemax={progress.total} aria-valuenow={progress.done}>
          <div style={{ width: `${(100 * progress.done) / progress.total}%` }} />
        </div>
      )}
    </div>
  );
}
