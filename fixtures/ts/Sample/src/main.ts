import { Order, OrderStatus } from "./domain/index.js";
import { InMemoryOrderRepository } from "./services/repository.js";
import { OrderService } from "./services/orderService.js";

async function main(): Promise<void> {
  const repository = new InMemoryOrderRepository();
  const service = new OrderService(repository);
  service.on("shipped", (order: Order) => {
    console.log(`Shipped ${order.id}`);
  });

  repository.add(new Order(1, "alice", 120, OrderStatus.Paid));
  repository.add(new Order(2, "bob", 40, OrderStatus.Pending));
  repository.add(new Order(3, "alice", 75, OrderStatus.Paid));

  const total = service.totalForCustomer("alice");
  console.log(`Total: ${total}`);

  for (const label of service.describeAll()) {
    console.log(label);
  }

  try {
    await service.ship(1);
    await service.ship(2);
  } catch (err) {
    console.log(`Could not ship: ${(err as Error).message}`);
  }
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
