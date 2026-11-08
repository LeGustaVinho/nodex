use crate::domain::Order;

/// Where orders are kept.
pub trait OrderRepository {
    fn all(&self) -> Vec<Order>;
    fn find(&self, id: u32) -> Option<Order>;
    fn add(&mut self, order: Order);
    fn replace(&mut self, order: Order);
}

/// Orders kept in memory.
#[derive(Default)]
pub struct InMemoryOrderRepository {
    orders: Vec<Order>,
}

impl InMemoryOrderRepository {
    pub fn new() -> Self {
        InMemoryOrderRepository { orders: Vec::new() }
    }
}

impl OrderRepository for InMemoryOrderRepository {
    fn all(&self) -> Vec<Order> {
        self.orders.clone()
    }

    fn find(&self, id: u32) -> Option<Order> {
        self.orders.iter().find(|o| o.id == id).cloned()
    }

    fn add(&mut self, order: Order) {
        self.orders.push(order);
    }

    fn replace(&mut self, order: Order) {
        if let Some(slot) = self.orders.iter_mut().find(|o| o.id == order.id) {
            *slot = order;
        }
    }
}
