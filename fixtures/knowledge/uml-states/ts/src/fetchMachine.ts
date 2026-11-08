import { createMachine } from "xstate";

export const fetchMachine = createMachine({
  id: "fetch",
  initial: "idle",
  states: {
    idle: {
      on: { FETCH: "loading" },
    },
    loading: {
      invoke: {
        src: "load",
        onDone: "success",
        onError: { target: "failure" },
      },
      on: { CANCEL: "idle" },
    },
    success: {
      initial: "fresh",
      states: {
        fresh: { after: { 60000: "stale" } },
        stale: { on: { REFRESH: "#fetch.loading" } },
      },
    },
    failure: {
      on: {
        RETRY: [{ target: "loading", guard: "canRetry" }, { target: "idle" }],
      },
    },
  },
});
