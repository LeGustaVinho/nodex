import { useEffect, useRef, useState } from "react";
import { isDesktop } from "../api";
import { useUi } from "../store";
import { useToasts } from "../toast";

/** Short messages in the bottom-right corner. */
export function Toasts() {
  const toasts = useToasts((s) => s.toasts);
  const dismiss = useToasts((s) => s.dismiss);
  if (toasts.length === 0) return null;
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.kind}`}>
          <span>{t.text}</span>
          <button className="modal-close" onClick={() => dismiss(t.id)} aria-label="Dismiss">
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}

/** How long the live connection may be down before the page says so (reconnects are quick). */
const GRACE_MS = 3000;

/** Covers the page while the connection to the Nodex server is lost. */
export function ConnectionOverlay() {
  const connected = useUi((s) => s.connected);
  const everConnected = useRef(false);
  const [lost, setLost] = useState(false);
  useEffect(() => {
    if (connected) {
      everConnected.current = true;
      setLost(false);
      return;
    }
    if (!everConnected.current) return;
    const timer = setTimeout(() => setLost(true), GRACE_MS);
    return () => clearTimeout(timer);
  }, [connected]);
  if (!lost) return null;
  return (
    <div className="modal-backdrop connection-lost" role="alert">
      <div className="modal small-modal">
        <div className="modal-head">
          <strong>Connection to Nodex lost</strong>
        </div>
        <p className="muted pad">
          {isDesktop
            ? "Reconnecting… If this does not go away, restart Nodex."
            : "Reconnecting… If the Nodex server was stopped, start it again and open the link it prints."}
        </p>
      </div>
    </div>
  );
}
