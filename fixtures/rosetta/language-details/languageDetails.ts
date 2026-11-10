import type { Counter } from "./counter";

export function explain(input: string | number, fallback = 4) {
  const settings = { step: 1 };
  settings.step += 1;
  const [first = fallback, ...others] = [0, settings.step];
  const chosen = first ?? fallback;
  const alternative = first || fallback;
  const snapshot = Object.freeze({ chosen, ...settings } as const);
  if (typeof input === "string") {
    return [input.length, ...others].map(n => n + snapshot.step);
  }
  return [alternative, input];
}

export function customMap(counter: Counter) {
  const custom = { map: (callback: (n: number) => number) => () => callback(counter.settings.step) };
  return custom.map(n => n + 1);
}


export function representation(value: number | object, structural: { length: number }) {
  if (typeof value === "number") return value;
  return { value, length: structural.length };
}

export function narrowedCatch() {
  try { throw 1; }
  catch (problem) {
    if (typeof problem === "number") return problem;
  }
}

export function narrowedIteration(list: (number | { n: number })[]) {
  for (const item of list) {
    if (typeof item === "number") return item;
  }
}

export function narrowedDestructure(input: { item: number | { n: number } }) {
  const { item } = input;
  if (typeof item === "number") return item;
}
