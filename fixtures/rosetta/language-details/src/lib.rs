pub mod universal_types;
pub mod rust_reading;
pub mod rust_reading_index;

pub(crate) fn standard_nested() -> std::collections::VecDeque<Vec<i32>> {
    std::collections::VecDeque::new()
}

pub fn explain(pair: (i32, i32), maybe: Option<i32>) -> Option<i32> {
    let (mut a, b) = pair;
    let r = &mut a;
    *r += 1;
    let b = b + 1;
    let increment = maybe?;
    let numbers = [a, b, increment];
    let result: Vec<_> = numbers.into_iter().map(|value| value + 1).collect();
    Some(result[0])
}


pub fn borrow_first<'a>(first: &'a i32, _other: &i32) -> &'a i32 {
    first
}

pub fn from_first_caller() -> i32 {
    let owner = 10;
    let other = 20;
    let result = borrow_first(&owner, &other);
    *result
}

pub fn from_second_caller() -> i32 {
    let separate = 30;
    let other = 40;
    let result = borrow_first(&separate, &other);
    *result
}


pub fn smart_owner() -> i32 {
    let strong: std::rc::Rc<i32> = std::rc::Rc::new(42);
    let borrowed = &*strong;
    *borrowed
}

pub struct Person { pub name: String }

pub fn borrow_field(owner: Person) -> usize {
    let borrowed = &owner.name;
    borrowed.len()
}

pub struct ViewData(i32);
static GLOBAL_VIEW: ViewData = ViewData(7);
pub struct ViewWrapper;

impl std::ops::Deref for ViewWrapper {
    type Target = ViewData;
    fn deref(&self) -> &ViewData { &GLOBAL_VIEW }
}

impl ViewData {
    pub fn view(&self) -> &i32 { &self.0 }
    pub fn relay(&self) -> &i32 { self.view() }
}

pub fn direct_receiver() -> i32 {
    let owner = ViewData(11);
    let borrowed = owner.view();
    *borrowed
}

pub fn inferred_reference_receiver() -> i32 {
    let owner = ViewData(15);
    let reference = &owner;
    let borrowed = reference.relay();
    *borrowed
}

pub fn reference_receiver(owner: &ViewData) -> i32 {
    let borrowed = owner.view();
    *borrowed
}

pub fn call_reference_receiver() -> i32 {
    let actual_owner = ViewData(12);
    reference_receiver(&actual_owner)
}

pub fn implicit_deref_receiver() -> i32 {
    let wrapper = ViewWrapper;
    let borrowed = wrapper.view();
    assert!(std::ptr::eq(borrowed, &GLOBAL_VIEW.0));
    *borrowed
}

pub fn mixed_receiver(flag: bool) -> i32 {
    let wrapper = ViewWrapper;
    let owner = 7;
    let borrowed = if flag { wrapper.view() } else { &owner };
    *borrowed
}

pub async fn async_borrow(value: &i32) -> &i32 { value }

pub async fn awaited_owner() -> i32 {
    let owner = 11;
    let borrowed = async_borrow(&owner).await;
    *borrowed
}

#[allow(unused_parens)]
pub fn grouped_borrow(value: (&i32)) -> &i32 { value }

pub fn grouped_owner() -> i32 {
    let owner = 12;
    let borrowed = grouped_borrow(&owner);
    *borrowed
}

pub fn paged_borrow(value: &i32) -> i32 { *value }

pub fn paged_callers(choice: usize) -> i32 {
    let a = 0; let b = 1; let c = 2; let d = 3; let e = 4;
    let f = 5; let g = 6; let h = 7; let i = 8; let j = 9;
    let k = 10; let l = 11; let m = 12; let n = 13; let o = 14;
    let p = 15; let q = 16; let r = 17; let s = 18; let t = 19;
    let value = match choice {
        0 => &a, 1 => &b, 2 => &c, 3 => &d, 4 => &e,
        5 => &f, 6 => &g, 7 => &h, 8 => &i, 9 => &j,
        10 => &k, 11 => &l, 12 => &m, 13 => &n, 14 => &o,
        15 => &p, 16 => &q, 17 => &r, 18 => &s, _ => &t,
    };
    paged_borrow(value)
        + paged_borrow(value) + paged_borrow(value) + paged_borrow(value) + paged_borrow(value)
        + paged_borrow(value) + paged_borrow(value) + paged_borrow(value) + paged_borrow(value)
        + paged_borrow(value) + paged_borrow(value) + paged_borrow(value) + paged_borrow(value)
        + paged_borrow(value) + paged_borrow(value) + paged_borrow(value) + paged_borrow(value)
        + paged_borrow(value) + paged_borrow(value) + paged_borrow(value) + paged_borrow(value)
        + paged_borrow(value) + paged_borrow(value) + paged_borrow(value) + paged_borrow(value)
}
