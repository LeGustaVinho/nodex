use std::fmt;

use crate::domain::{Order, OrderEvent, OrderStatus};
use crate::services::OrderRepository;
use crate::util::sleep_ms;

/// Why an order could not ship.
#[derive(Debug)]
pub enum ShipError {
    NotFound(u32),
    NotPaid(u32),
}

impl fmt::Display for ShipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShipError::NotFound(id) => write!(f, "Order {id} not found"),
            ShipError::NotPaid(_) => write!(f, "Only paid orders can ship"),
        }
    }
}

type Listener = Box<dyn Fn(&Order)>;

/// Totals, descriptions and shipping for the orders of a repository.
pub struct OrderService<R: OrderRepository> {
    repository: R,
    listeners: Vec<Listener>,
}

impl<R: OrderRepository> OrderService<R> {
    pub fn new(repository: R) -> Self {
        OrderService { repository, listeners: Vec::new() }
    }

    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }

    /// Runs the listener every time an order ships.
    pub fn on_shipped(&mut self, listener: Listener) {
        self.listeners.push(listener);
    }

    pub fn total_for_customer(&self, customer: &str) -> i64 {
        let mut total = 0;
        for order in self.repository.all() {
            if order.customer != customer {
                continue;
            }
            if order.status == OrderStatus::Cancelled {
                break;
            }
            total += order.amount;
        }
        total
    }

    pub fn total_for_customer_pipeline(&self, customer: &str) -> i64 {
        self.repository
            .all()
            .iter()
            .filter(|o| o.customer == customer)
            .map(|o| o.amount)
            .sum()
    }

    pub fn describe_all(&self) -> Vec<String> {
        let mut labels = Vec::new();
        for i in 0..3 {
            let order = self.repository.find(i + 1);
            let label = match order.map(|o| o.status) {
                Some(OrderStatus::Paid) => "paid",
                Some(OrderStatus::Pending) => "waiting",
                None => "missing",
                _ => "other",
            };
            labels.push(format!("#{}: {label}", i + 1));
        }
        labels
    }

    pub fn ship(&mut self, id: u32) -> Result<(), ShipError> {
        let Some(order) = self.repository.find(id) else {
            return Err(ShipError::NotFound(id));
        };
        if order.status != OrderStatus::Paid {
            return Err(ShipError::NotPaid(id));
        }

        sleep_ms(10);
        let shipped = order.with_status(OrderStatus::Shipped);
        self.repository.replace(shipped.clone());
        for listener in &self.listeners {
            listener(&shipped);
        }
        Ok(())
    }

    pub fn describe(&self, event: &OrderEvent) -> String {
        match event {
            OrderEvent::Created { id } => format!("order {id} created"),
            OrderEvent::Shipped { id, at } => format!("order {id} shipped at {at}"),
            OrderEvent::Cancelled { id, reason } => format!("order {id} cancelled: {reason}"),
        }
    }

    pub fn count_pending(&self) -> usize {
        let mut count = 0;
        let all = self.repository.all();
        let mut index = 0;
        while index < all.len() {
            if all[index].status == OrderStatus::Pending {
                count += 1;
            }
            index += 1;
        }
        count
    }
}
