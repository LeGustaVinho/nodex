//! Nodex Rust sample: the same orders program as the C# and TypeScript samples.
#![allow(dead_code, unused_imports)]

mod domain;
mod services;
mod util;

use domain::{Order, OrderStatus};
use services::{InMemoryOrderRepository, OrderRepository, OrderService, ShipError};

fn main() {
    let repository = InMemoryOrderRepository::new();
    let mut service = OrderService::new(repository);
    service.on_shipped(Box::new(|order: &Order| {
        println!("Shipped {}", order.id);
    }));

    service.repository_mut().add(Order::new(1, "alice", 120, OrderStatus::Paid));
    service.repository_mut().add(Order::new(2, "bob", 40, OrderStatus::Pending));
    service.repository_mut().add(Order::new(3, "alice", 75, OrderStatus::Paid));

    let total = service.total_for_customer("alice");
    println!("Total: {total}");

    for label in service.describe_all() {
        println!("{label}");
    }

    if let Err(err) = ship_all(&mut service) {
        println!("Could not ship: {err}");
    }

    let worker = std::thread::spawn(move || {
        let pending = util::count_slowly(3);
        pending * 2
    });
    let doubled = worker.join().unwrap_or(0);
    println!("Worker said {doubled}");
}

fn ship_all(service: &mut OrderService<InMemoryOrderRepository>) -> Result<(), ShipError> {
    service.ship(1)?;
    service.ship(2)?;
    Ok(())
}
