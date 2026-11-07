import type { Entity } from "./order.js";

export abstract class Shape implements Entity {
  constructor(public readonly id: number) {}

  abstract area(): number;

  describe(): string {
    return `${this.constructor.name} with area ${this.area().toFixed(2)}`;
  }
}

export class Circle extends Shape {
  readonly #radius: number;

  constructor(id: number, radius: number) {
    super(id);
    if (radius <= 0) {
      throw new RangeError("radius must be positive");
    }
    this.#radius = radius;
  }

  area(): number {
    return Math.PI * this.#radius * this.#radius;
  }
}

export class Rectangle extends Shape {
  width = 0;
  height = 0;

  area(): number {
    return this.width * this.height;
  }
}

/** What happened to an order, as a discriminated union. */
export type OrderEvent =
  | { kind: "created"; id: number }
  | { kind: "shipped"; id: number; at: Date }
  | { kind: "cancelled"; id: number; reason: string };
