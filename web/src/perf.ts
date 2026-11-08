// Timings of opening a project: `nodex:` performance marks (DevTools → Performance) and
// `[nodex perf]` lines in the console (verbose level), measured from `project-loading`.

let openedAt: number | null = null;
let firstViewShown = false;

const since = () => (openedAt === null ? "" : ` at +${(performance.now() - openedAt).toFixed(0)} ms`);

/** A project started loading (any path: Welcome, Open dialog, menu, drag and drop). */
export function openStarted() {
  openedAt = performance.now();
  firstViewShown = false;
  performance.mark("nodex:open-start");
  console.debug("[nodex perf] open-start");
}

/** A step of the open (`project-changed`, …). */
export function step(name: string) {
  performance.mark(`nodex:${name}`);
  console.debug(`[nodex perf] ${name}${since()}`);
}

/** The first view has data on screen; logged once per open, after the next paint. */
export function firstView(view: string) {
  if (firstViewShown || openedAt === null) return;
  firstViewShown = true;
  requestAnimationFrame(() => setTimeout(() => step(`first-view:${view}`), 0));
}

/** Logs how long a large payload took to download and to parse. */
export function payload(name: string, bytes: number, downloadMs: number, parseMs: number) {
  console.debug(
    `[nodex perf] ${name}: ${(bytes / 1_048_576).toFixed(2)} MB, download ${downloadMs.toFixed(0)} ms, parse ${parseMs.toFixed(0)} ms${since()}`,
  );
}

/** Times `work` and logs it when it takes more than a few milliseconds. */
export function timed<T>(name: string, work: () => T): T {
  const started = performance.now();
  const value = work();
  const ms = performance.now() - started;
  if (ms > 5) console.debug(`[nodex perf] ${name}: ${ms.toFixed(0)} ms${since()}`);
  return value;
}
