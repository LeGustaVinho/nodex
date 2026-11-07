// TypeScript 4.7–5.9 and ES2022–2025 syntax that tree-sitter-typescript 0.23 does not parse
// directly (bridged by crates/nodex-lang-typescript/src/prepare.rs), plus other recent forms.
import config from "./modules/defaults.js" with { type: "json" };
import * as modules from "./modules/index.js";
import Helper = modules.helper;
export type * from "./modules/index.js";
export * as everything from "./modules/index.js";

export interface Box<in out T> {
  value: T;
}

export class Holder<out O = unknown, in I = never> {
  read(): O | undefined {
    return undefined;
  }
  write(_value: I): void {}
}

export function tuple<const T extends readonly unknown[]>(...items: T): T {
  return items;
}

function Inject(_token: string) {
  return (_target: object, _key: string | symbol | undefined, _index: number) => {};
}

export abstract class Service {
  abstract override toString(): string;
  protected abstract readonly name: string;
  static #instances = 0;
  #token = "secret";

  constructor(@Inject("DB") readonly db: string, @Inject("LOG") private readonly log: string) {
    Service.#instances++;
  }

  owns(other: object): boolean {
    if (#token in other) {
      return true;
    }
    return false;
  }

  static count(): number {
    return Service.#instances;
  }
}

declare function openAll(): Disposable[];

export async function resources(): Promise<number> {
  let closed = 0;
  for (using r of openAll()) {
    closed += r ? 1 : 0;
  }
  for (await using a of openAll()) {
    closed += a ? 1 : 0;
  }
  const using = { bind: (n: number) => n + closed };
  return using.bind(1) + Helper().length + String(config).length;
}

export function view(): string {
  const markup = `<p>&nbsp;</p>`;
  return markup;
}

declare global {
  interface Window {
    nodexModern: boolean;
  }
}

export type Shape = { kind: "circle"; r: number } | { kind: "square"; side: number };

export function area(shape: Shape): number {
  switch (shape.kind) {
    case "circle":
      return Math.PI * shape.r ** 2;
    case "square":
      return shape.side ** 2;
  }
  shape satisfies never;
  const unreachable: never = shape;
  return unreachable;
}

export const Mixed = class {
  greet(): string {
    return "hi";
  }
};

declare module "./modules/a.js" {
  export function extra(): number;
}

(async () => {
  const ready = await Promise.resolve(true);
  if (!ready) {
    throw new Error("not ready");
  }
})();
