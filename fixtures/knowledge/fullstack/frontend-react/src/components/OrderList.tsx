import React, { useEffect, useState } from "react";
import { cancelOrder, getOrders, OrderDto } from "../api/orders";
import { OrderRow } from "./OrderRow";

export function OrderList() {
  const [orders, setOrders] = useState<OrderDto[]>([]);

  useEffect(() => {
    getOrders().then(setOrders);
  }, []);

  const onCancel = async (id: string) => {
    try {
      await cancelOrder(id);
    } catch (e) {
      console.error(e);
    }
  };

  const refresh = async () => {
    setOrders(await getOrders());
  };

  return (
    <div>
      <button onClick={refresh}>Refresh</button>
      <ul>
        {orders.map((o) => (
          <OrderRow key={o.id} order={o} onCancel={onCancel} />
        ))}
      </ul>
    </div>
  );
}
