export enum OrderStatus {
  Pending,
  Paid,
  Shipped,
  Cancelled,
}

/** Anything stored by id. */
export interface Entity {
  readonly id: number;
}

/** Anything with a buyer and an amount. Orders fit it without declaring it (structural typing). */
export interface Priced {
  readonly customer: string;
  readonly amount: number;
}

/** A customer order. Immutable: changes create a copy. */
export class Order implements Entity {
  constructor(
    public readonly id: number,
    public readonly customer: string,
    public readonly amount: number,
    public readonly status: OrderStatus,
  ) {}

  get isLarge(): boolean {
    return this.amount >= 100;
  }

  withStatus(status: OrderStatus): Order {
    return new Order(this.id, this.customer, this.amount, status);
  }
}
