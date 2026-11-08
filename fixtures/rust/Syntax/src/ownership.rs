//! Ownership: moves, borrows, smart pointers, Drop and shadowing.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct Guard {
    pub name: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        println!("closing {}", self.name);
    }
}

pub fn ownership(text: String) -> usize {
    let owned = text;
    let borrowed = &owned;
    let len = borrowed.len();
    let mut buffer = String::new();
    let writer = &mut buffer;
    writer.push_str(&owned);
    let moved = buffer;
    let copied = len;
    let again = copied;
    let cloned = moved.clone();
    let boxed: Box<u32> = Box::new(5);
    let shared = Rc::new(RefCell::new(Vec::<u32>::new()));
    let other = Rc::clone(&shared);
    other.borrow_mut().push(*boxed);
    let counter = Arc::new(Mutex::new(0));
    {
        let mut lock = counter.lock().unwrap();
        *lock += 1;
    }
    let value = 1;
    let value = value + 1;
    let value = value * 10;
    let guard = Guard { name: cloned };
    drop(guard);
    again + value + shared.borrow().len()
}

pub fn scoped() {
    let _outer = Guard { name: "outer".into() };
    {
        let _inner = Guard { name: "inner".into() };
    }
}
