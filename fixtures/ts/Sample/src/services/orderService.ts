import { EventEmitter } from "node:events";
import { Order, OrderEvent, OrderStatus } from "../domain/index.js";
import type { OrderRepository } from "./repository.js";
import { sleep } from "../util/timing.js";

export class OrderService extends EventEmitter {
  constructor(private readonly repository: OrderRepository) {
    super();
  }

  totalForCustomer(customer: string): number {
    let total = 0;
    for (const order of this.repository.all()) {
      if (order.customer !== customer) {
        continue;
      }
      if (order.status === OrderStatus.Cancelled) {
        break;
      }
      total += order.amount;
    }
    return total;
  }

  totalForCustomerPipeline(customer: string): number {
    return [...this.repository.all()]
      .filter((o) => o.customer === customer)
      .map((o) => o.amount)
      .reduce((sum, amount) => sum + amount, 0);
  }

  *describeAll(): Generator<string> {
    for (let i = 0; i < 3; i++) {
      const order = this.repository.find(i + 1);
      let label: string;
      switch (order?.status) {
        case OrderStatus.Paid:
          label = "paid";
          break;
        case OrderStatus.Pending:
          label = "waiting";
          break;
        case undefined:
          label = "missing";
          break;
        default:
          label = "other";
      }
      yield `#${i + 1}: ${label}`;
    }
  }

  async ship(id: number): Promise<void> {
    const order = this.repository.find(id) ?? fail(`Order ${id} not found`);
    if (order.status !== OrderStatus.Paid) {
      throw new Error("Only paid orders can ship");
    }

    await sleep(10);
    const shipped = order.withStatus(OrderStatus.Shipped);
    this.repository.replace(shipped);
    this.emit("shipped", shipped);
  }

  describe(event: OrderEvent): string {
    switch (event.kind) {
      case "created":
        return `order ${event.id} created`;
      case "shipped":
        return `order ${event.id} shipped at ${event.at.toISOString()}`;
      case "cancelled":
        return `order ${event.id} cancelled: ${event.reason}`;
    }
  }

  countPending(): number {
    let count = 0;
    const all = [...this.repository.all()];
    let index = 0;
    while (index < all.length) {
      if (all[index].status === OrderStatus.Pending) {
        count++;
      }
      index++;
    }
    return count;
  }
}

function fail(message: string): never {
  throw new Error(message);
}
