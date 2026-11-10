import { useEffect, useRef } from "react";
import type { Span } from "../gen/Span";
import { useUi } from "../store";

/** Follow each replay step once, even if graph data arrives in a later render. */
export function useReplaySource(step: object | null, span: Span | undefined) {
  const pending = useRef<{ step: object; intent: number; followed: boolean } | null>(null);
  useEffect(() => {
    if (!step) {
      pending.current = null;
      return;
    }
    const state = useUi.getState();
    if (pending.current?.step !== step) pending.current = { step, intent: state.codeIntent, followed: false };
    if (!span || pending.current.followed || pending.current.intent !== state.codeIntent) return;
    pending.current.followed = true;
    state.focusCode(span);
  }, [step, span]);
}
