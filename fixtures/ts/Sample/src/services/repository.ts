import { Order } from "../domain/index.js";

export interface OrderRepository {
  all(): Iterable<Order>;
  find(id: number): Order | undefined;
  add(order: Order): void;
  replace(order: Order): void;
}

export class InMemoryOrderRepository implements OrderRepository {
  private readonly orders = new Map<number, Order>();

  *all(): Iterable<Order> {
    yield* this.orders.values();
  }

  find(id: number): Order | undefined {
    return this.orders.get(id);
  }

  add(order: Order): void {
    this.orders.set(order.id, order);
  }

  replace = (order: Order): void => {
    this.orders.set(order.id, order);
  };
}
