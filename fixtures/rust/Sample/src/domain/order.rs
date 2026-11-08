use std::fmt;

/// Where an order is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,
    Paid,
    Shipped,
    Cancelled,
}

/// A customer order. Immutable: changes create a copy.
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub id: u32,
    pub customer: String,
    pub amount: i64,
    pub status: OrderStatus,
}

/// Something that happened to an order.
#[derive(Debug, Clone)]
pub enum OrderEvent {
    Created { id: u32 },
    Shipped { id: u32, at: u64 },
    Cancelled { id: u32, reason: String },
}

/// Anything with a buyer and an amount.
pub trait Priced {
    fn customer(&self) -> &str;
    fn amount(&self) -> i64;

    fn is_large(&self) -> bool {
        self.amount() >= 100
    }
}

impl Order {
    pub fn new(id: u32, customer: &str, amount: i64, status: OrderStatus) -> Self {
        Order { id, customer: customer.to_owned(), amount, status }
    }

    pub fn with_status(&self, status: OrderStatus) -> Order {
        Order { status, ..self.clone() }
    }
}

impl Priced for Order {
    fn customer(&self) -> &str {
        &self.customer
    }

    fn amount(&self) -> i64 {
        self.amount
    }
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{} {} {}", self.id, self.customer, self.amount)
    }
}
