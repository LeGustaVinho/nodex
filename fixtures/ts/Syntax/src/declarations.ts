import { helper, type Shape as ShapeType } from "./modules/index.js";
import * as path from "node:path";
import defaultExport from "./modules/defaults.js";
import fs = require("node:fs");

export enum Color {
  Red = "red",
  Green = "green",
}

export const enum Flags {
  None = 0,
  A = 1 << 0,
  B = 1 << 1,
}

/** Something with an area. */
export interface Shape {
  readonly name: string;
  area(): number;
  describe?(): string;
  [key: string]: unknown;
}

// Declaration merging: a second block adds members.
export interface Shape {
  tags?: string[];
}

export interface Named<T extends string = string> extends Partial<Shape> {
  label: T;
}

export type Id = string | number;
export type Pair<A, B> = readonly [A, B];
export type Handler = (event: Event) => void;
export type Mapped<T> = { readonly [K in keyof T]?: T[K] };
export type Result<T> =
  | { ok: true; value: T }
  | { ok: false; error: Error };

function sealed(target: unknown, context: ClassDecoratorContext) {
  void target;
  void context;
}

function logged(_target: unknown, context: ClassMethodDecoratorContext) {
  return function (this: unknown, ...args: unknown[]) {
    console.log(String(context.name), args.length);
  };
}

@sealed
export abstract class Base<T> implements Shape {
  static count = 0;
  static {
    Base.count = 1;
  }
  protected abstract readonly kind: string;
  #secret = 42;
  declare tags?: string[];
  accessor size = 1;
  [key: string]: unknown;

  constructor(public readonly name: string, protected value?: T) {
    Base.count++;
  }

  abstract area(): number;

  get secret(): number {
    return this.#secret;
  }

  set secret(v: number) {
    this.#secret = v;
  }

  #privateHelper(): number {
    return this.#secret * 2;
  }

  @logged
  describe(): string {
    return `${this.name}: ${this.area()}`;
  }

  static create<U>(this: new (name: string) => U, name: string): U {
    return new this(name);
  }
}

export class Square extends Base<number> {
  protected readonly kind = "square";

  override area(): number {
    return (this.value ?? 0) ** 2;
  }

  override describe(): string {
    return super.describe() + "!";
  }

  onResize = (factor: number): void => {
    this.value = (this.value ?? 0) * factor;
  };
}

type Ctor<T = object> = new (...args: any[]) => T;

export function Timestamped<TBase extends Ctor>(B: TBase) {
  return class extends B {
    timestamp = Date.now();
  };
}

export function overload(value: string): string;
export function overload(value: number): number;
export function overload(value: string | number): string | number {
  return value;
}

export function* counter(limit: number): Generator<number, void, unknown> {
  for (let i = 0; i < limit; i++) {
    yield i;
  }
}

export async function* ticks(): AsyncGenerator<number> {
  yield 1;
}

export const arrow = <T,>(items: T[]): T | undefined => items[0];

export namespace Geometry {
  export const unit = 1;
  export function scale(v: number): number {
    return v * unit;
  }
  export namespace Nested {
    export type Point = { x: number; y: number };
  }
}

declare global {
  interface Window {
    nodex?: unknown;
  }
}

declare module "virtual:config" {
  export const value: string;
}

export default class {
  run(): string {
    return path.join(helper(), defaultExport, String(fs));
  }
}

export { Square as Box };
export type { ShapeType };
