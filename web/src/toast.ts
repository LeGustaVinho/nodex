import { create } from "zustand";

export type ToastKind = "info" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
}

interface ToastState {
  toasts: Toast[];
  push: (kind: ToastKind, text: string) => void;
  dismiss: (id: number) => void;
}

/** How long a toast stays, by kind (errors stay longer: they may need reading). */
const LIFETIME: Record<ToastKind, number> = { info: 4000, error: 9000 };

let nextId = 1;

export const useToasts = create<ToastState>((set, get) => ({
  toasts: [],
  push: (kind, text) => {
    const id = nextId++;
    set({ toasts: [...get().toasts.slice(-3), { id, kind, text }] });
    setTimeout(() => get().dismiss(id), LIFETIME[kind]);
  },
  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
}));

/** Short messages in the bottom-right corner, in place of `alert()`. */
export const toast = {
  info: (text: string) => useToasts.getState().push("info", text),
  error: (text: string) => useToasts.getState().push("error", text),
};

/** The message of an API or other error, without the leading HTTP status. */
export function errorText(e: unknown): string {
  return (e instanceof Error ? e.message : String(e)).replace(/^\d{3} /, "");
}
