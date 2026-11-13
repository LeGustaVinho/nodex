use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::rust_reading_index::{Encounter, ReadingCrossFile};

pub fn prepare_crew(encounter: &Encounter, crew: &str) -> Option<usize> {
    let host = |crew: &str| encounter.crew_index.get(crew).map(|&(host, _)| host);
    host(crew)
}

pub fn find_local(crew: &str) -> Option<usize> {
    let owner: HashMap<String, (usize, usize)> = HashMap::new();
    HashMap::get(&owner, crew).map(|&(host, _)| host)
}

pub fn supplied_by_caller(crew: &str) -> Option<usize> {
    let encounter = Encounter {
        crew_index: HashMap::from([("captain".into(), (3, 2))]),
    };
    prepare_crew(&encounter, crew)
}

pub fn from_caller_only(encounter: &Encounter, crew: &str) -> Option<usize> {
    let id = |crew: &str| encounter.crew_index.get(crew).map(|&(id, _)| id);
    id(crew)
}

pub fn keep_borrow<'a>(map: &'a mut HashMap<&str, String>, key: &str) -> Option<&'a mut String> {
    map.get_mut(key).map(|entry| {
        return entry;
    })
}

pub fn reborrow_entry<'a>(map: &'a HashMap<&str, (usize, usize)>, key: &str) -> Option<&'a usize> {
    map.get(key).and_then(|entry| Some(&entry.0))
}

pub fn unpack_entry<'a>(map: &'a HashMap<&str, (usize, usize)>, key: &str) -> Option<&'a usize> {
    map.get(key).map(|&(ref item, _)| item)
}

pub fn changed_capture<'a>(a: &'a i32, b: &'a i32) -> Option<&'a i32> {
    let mut selected = a;
    Some(()).map(|_| {
        selected = b;
    });
    Some(()).map(|_| selected)
}

pub fn borrow_split(values: &mut [i32]) {
    let (ref before, ref after) = values.split_at_mut(1);
    let _ = (before, after);
}

const LIMIT: i32 = 1;

pub fn match_value(x: i32, y: i32) -> i32 {
    let result = match (x, y) {
        (LIMIT, value) if value > 0 => value,
        _ => 0,
    };
    result
}

pub fn shadowed_callback(map: &HashMap<&str, (usize, usize)>, key: &str) -> Option<usize> {
    map.get(key).map(|&(host, _)| {
        let host = host + 1;
        host
    })
}

pub fn may_exit_callback(values: &[Option<i32>]) -> Vec<Option<i32>> {
    values.iter().map(|item| Some((*item)? + 1)).collect()
}

pub fn fallible(value: Option<Result<i32, &'static str>>) -> Result<Option<i32>, String> {
    let value = value.transpose().map_err(str::to_owned)?;
    let Some(value) = value.filter(|&n| n > 0) else {
        return Ok(None);
    };
    Ok(Some(value))
}

pub fn defaults(value: Option<i32>, fallback: fn() -> i32) -> i32 {
    let eager = value.unwrap_or(fallback());
    let lazy = value.unwrap_or_else(fallback);
    let result = value.ok_or_else(|| "absent");
    eager + lazy + result.expect("present")
}

pub fn split(values: &mut [i32], mid: usize) {
    let (before, after) = values.split_at_mut(mid);
    if let Some(first) = before.first_mut() {
        *first += 1;
    }
    match after {
        [first, ..] => *first += 1,
        _ => {}
    }
}

pub trait Label {
    fn describe<'a, T: AsRef<str>>(&self, value: &'a T) -> &'a str
    where
        T: 'a;
}

#[derive(Clone, Copy)]
pub struct Number(pub i32);

#[cfg(any(unix, windows))]
pub fn trait_examples(
    source: impl AsRef<str>,
    fallback: &dyn AsRef<str>,
) -> impl Iterator<Item = i32> {
    let count = source
        .as_ref()
        .parse::<i32>()
        .unwrap_or_else(|_| fallback.as_ref().len() as i32);
    0..count
}

macro_rules! project_macro {
    ($x:expr) => {
        $x + 1
    };
}

pub fn macros() -> String {
    let one = project_macro!(1);
    let numbers = vec![one];
    assert!(!numbers.is_empty());
    format!("{numbers:?}")
}

pub async fn get_number() -> i32 {
    2
}

pub async fn deferred() -> i32 {
    let count = 2;
    let task = async move { get_number().await + count };
    task.await
}

pub fn protected() -> i32 {
    let shared = Arc::new(Mutex::new(0));
    let other = Arc::clone(&shared);
    let thread = std::thread::spawn(move || {
        *other.lock().unwrap() = 5;
    });
    thread.join().unwrap();
    let lock = RwLock::new(3);
    let _guard = lock.read().unwrap();
    *shared.lock().unwrap()
}

pub struct Custom;
impl Custom {
    fn get(&self, _key: usize) -> Self {
        Self
    }
    fn map(self, _callback: fn(i32) -> i32) -> i32 {
        42
    }
    fn unwrap(self) -> i32 {
        42
    }
}

pub fn not_a_lookup() -> i32 {
    Custom.get(0).map(|value| value + 1) + Custom.unwrap()
}

pub fn synchronous_spawn() -> i32 {
    mod thread {
        pub fn spawn(work: impl FnOnce() -> i32) -> i32 {
            work()
        }
    }
    thread::spawn(|| 1);
    let value = thread::spawn(|| 42);
    value
}

pub fn effects_and_slices(values: &mut [i32], slot: &mut Option<i32>) -> i32 {
    let chosen = slot.take().unwrap_or_else(|| 5);
    let mut totals = HashMap::<i32, i32>::new();
    let total = totals.entry(chosen).or_default();
    *total += 1;
    let (before, after) = values.split_at_mut(1);
    let first = &before[0];
    after[0] += *first;
    after.iter().copied().sum::<i32>() + *total
}

pub fn ordinary_effect_names() -> i32 {
    fn drop(value: i32) -> i32 { value }
    fn exit() -> i32 { 1 }
    fn sleep() -> i32 { 2 }
    struct Counter;
    impl Counter {
        fn send(&self, value: i32) -> i32 { value }
    }
    drop(3);
    exit();
    sleep();
    Counter.send(4);
    10
}

pub fn ownership_labels(from_caller: &i32, change: bool) -> i32 {
    let left = (1, 2);
    let mut right = 3;
    let view = &left.0;
    let alias = view;
    let mut cursor = &left.0;
    if change {
        cursor = &right;
    }
    let picked = *cursor;
    let writer = &mut right;
    *writer += *alias + picked + *from_caller;
    right
}

pub struct MapView<'a>(&'a HashMap<&'a str, usize>);

impl<'a> std::ops::Deref for MapView<'a> {
    type Target = HashMap<&'a str, usize>;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

pub fn opaque_receiver(view: MapView<'_>, key: &str) -> Option<usize> {
    view.get(key).copied()
}

struct BorrowedSlice<'a>(&'a mut [i32]);

impl std::ops::Deref for BorrowedSlice<'_> {
    type Target = [i32];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl std::ops::DerefMut for BorrowedSlice<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

pub fn split_through_wrapper(input: &mut [i32]) -> i32 {
    let mut wrapper = BorrowedSlice(input);
    let (first, second) = wrapper.split_at_mut(1);
    first[0] + second[0]
}

pub fn capture_only_field() -> String {
    let pair = (String::from("retained"), 3_u32);
    let callback = move || pair.1;
    assert_eq!(callback(), 3);
    pair.0
}

pub struct Cleanup;

impl Cleanup {
    fn read(&self) -> i32 { 7 }
}

impl Drop for Cleanup {
    fn drop(&mut self) {}
}

pub fn cleanup_on_return(early: bool) -> i32 {
    let outer: Cleanup = Cleanup;
    if early {
        let inner: Cleanup = Cleanup;
        return inner.read() + outer.read();
    }
    outer.read()
}

pub fn return_owned_cleanup_value() -> Cleanup {
    let local: Cleanup = Cleanup;
    local
}

pub fn reading_ranges(values: &[i32], start: usize, end: usize) -> [&[i32]; 6] {
    [
        &values[start..end],
        &values[start..=end],
        &values[..end],
        &values[..=end],
        &values[start..],
        &values[..],
    ]
}

pub fn reading_inclusive_end(end: usize) -> std::ops::RangeToInclusive<usize> {
    ..=end
}

pub fn reading_explicit_return_range() -> std::ops::RangeInclusive<usize> {
    return 1..=3;
}

pub fn reading_boolean_checks(left: bool, right: fn() -> bool) -> (bool, bool, bool) {
    (left && right(), left || right(), left & right())
}

pub fn reading_let_conditions(mut items: Vec<i32>, minimum: i32) -> i32 {
    let mut total = 0;
    while let Some(value) = items.pop() && value >= minimum {
        total += value;
    }
    if let Some(value) = items.pop() && value > minimum && let Some(next) = items.pop() && next > value {
        total += next;
    }
    while let Some(value) = items.pop() {
        total += value;
    }
    total
}

pub fn reading_shadowed_conditions(input: Option<i32>, x: i32, y: i32) -> i32 {
    let mut total = 0;
    if let Some(x) = input && x > 0 {
        total += x;
    }
    let mut values = 0..2;
    while let Some(y) = values.next() && y > 0 {
        total += y;
    }
    if let Some(x) = Some(x) && x > 0 {
        total += x;
    }
    total + y
}

pub fn reading_let_else(input: Option<i32>, candidates: &[Option<i32>]) -> i32 {
    let Some(value) = input else { return -1; };
    for candidate in candidates {
        let Some(_) = candidate else { continue; };
        return value;
    }
    0
}

pub fn reading_let_else_closure(input: Option<i32>) -> i32 {
    let convert_with_default = |input: Option<i32>| {
        let Some(value) = input else { return 0; };
        value
    };
    convert_with_default(input)
}

pub trait ReadingItemSource<Tag> {
    type Item: Clone;
}

pub trait ReadingOtherSource {
    type Item;
}

pub struct ReadingSource;

impl ReadingItemSource<u16> for ReadingSource {
    type Item = String;
}

impl ReadingOtherSource for ReadingSource {
    type Item = bool;
}

impl ReadingOtherSource for ReadingCrossFile {
    type Item = usize;
}

impl ReadingItemSource<u8> for ReadingSource {
    type Item = i32;
}

pub mod reading_left {
    pub trait QualifiedSource { type Item; }
}

pub mod reading_right {
    pub trait QualifiedSource { type Item; }
}

impl reading_left::QualifiedSource for ReadingSource {
    type Item = u32;
}

impl reading_right::QualifiedSource for ReadingSource {
    type Item = u64;
}

pub fn reading_associated_requirement<I>(source: I) -> I
where
    I: ReadingItemSource<u8, Item = i32>,
{
    let _ = std::mem::size_of::<Box<dyn Iterator<Item = usize>>>();
    source
}

pub fn reading_static_lifetime(_: &'static str) -> &'static str {
    "fixed"
}

pub fn reading_elided_lifetime(text: &str, until: usize) -> &str {
    &text[..until]
}

pub fn reading_placeholder_lifetime(text: &str, until: usize) -> &'_ str {
    &text[..until]
}

pub fn reading_named_lifetime<'text>(text: &'text str) -> &'text str {
    text
}

impl ReadingSource {
    pub fn reading_receiver_lifetime(&self, other: &str) -> &str {
        let _ = other;
        "fixed"
    }
}

pub fn reading_collect_arguments(values: &[Option<i32>]) -> Option<Vec<i32>> {
    values.iter().copied().collect::<Option<Vec<_>>>()
}

struct ReadingCustomCollector;

impl ReadingCustomCollector {
    fn collect<const SIZE: usize>(&self) -> [(); SIZE] {
        [(); SIZE]
    }
}

pub fn reading_collect_const_argument() -> [(); 2] {
    ReadingCustomCollector.collect::<_>()
}

pub fn reading_collect_const_block() -> [(); 2] {
    ReadingCustomCollector.collect::<{ let _ = 1; 2 }>()
}

struct ReadingCustomTypeCollector;

impl ReadingCustomTypeCollector {
    fn collect<T: Default>(&self) -> T {
        T::default()
    }
}

pub fn reading_collect_array_length() -> [u8; 2] {
    ReadingCustomTypeCollector.collect::<[u8; _]>()
}

pub fn reading_collect_standard_option(values: &[Option<i32>]) -> Option<Vec<i32>> {
    values.iter().map(|value| value.as_ref().copied()).collect::<Option<Vec<_>>>()
}

struct ReadFirst(Option<i32>);

impl FromIterator<i32> for ReadFirst {
    fn from_iter<I: IntoIterator<Item = i32>>(values: I) -> Self {
        Self(values.into_iter().next())
    }
}

pub fn reading_collect_custom_from_iter(values: &[Option<i32>]) -> Option<i32> {
    let first = values.iter().map(|value| value.as_ref().copied()).collect::<Option<ReadFirst>>()?;
    first.0
}

pub mod local_option {
    pub struct Option<T>(pub T);

    impl<T> FromIterator<std::option::Option<T>> for Option<Vec<T>> {
        fn from_iter<I: IntoIterator<Item = std::option::Option<T>>>(values: I) -> Self {
            Self(values.into_iter().flatten().collect())
        }
    }

    pub fn reading_collect_local_option(values: &[std::option::Option<i32>]) -> Option<Vec<i32>> {
        values.iter().map(|value| value.as_ref().copied()).collect::<Option<Vec<_>>>()
    }
}

pub fn reading_closure_shared_result(values: &mut [[i32; 2]], position: &(usize, usize)) -> i32 {
    Some(position).map(|&(row, column)| &values[row][column]).copied().unwrap_or_default()
}

pub fn reading_closure_mutable_result(values: &mut [i32]) -> &mut i32 {
    Some(values).map(|values| &mut values[0]).unwrap()
}

pub fn reading_matches_status(status: Option<&str>) -> bool {
    matches!(status, Some("space" | "returning"))
}

pub fn reading_matches_shadowed(value: Option<i32>) -> bool {
    macro_rules! matches {
        ($ignored:expr, $pattern:pat) => { false };
    }
    let local = matches!(value, Some(_));
    let standard = std::matches!(value, Some(count) if count > 0);
    standard || local
}

pub fn reading_heading_cast(heading: Option<f64>) -> u16 {
    let full_turn = std::f64::consts::TAU;
    let turn = ((heading.unwrap_or(0.0) % full_turn) + full_turn) % full_turn;
    crate::rust_reading_index::reading_round(turn / full_turn * 65_535.0) as u16
}

pub struct ReadingCastField {
    pub value: f64,
}

pub fn reading_field_cast(input: ReadingCastField) -> i64 {
    input.value as i64
}

pub mod reading_cast_shadows {
    #![allow(non_camel_case_types)]

    type u8 = *const i32;
    type f64 = i32;

    pub fn reading_alias_target_cast() -> u8 {
        1_u64 as u8
    }

    fn aliased_input() -> f64 {
        1
    }

    pub fn reading_alias_source_cast() -> u16 {
        aliased_input() as u16
    }
}

pub mod reading_cast_nominal {
    #[allow(non_camel_case_types)]
    pub enum u8 {
        Idle,
    }

    pub fn reading_nominal_source_cast(value: u8) -> u64 {
        value as u64
    }

    pub fn reading_inferred_nominal_source_cast() -> u64 {
        let value = u8::Idle;
        value as u64
    }

    pub fn reading_qualified_primitive_source_cast(value: std::primitive::u8) -> u64 {
        value as u64
    }

    pub fn reading_inferred_primitive_source_cast() -> u64 {
        let value = 20_u8;
        value as u64
    }
}

pub fn reading_result_ok(raw: i64, records: &HashMap<u32, i32>) -> Option<i32> {
    u32::try_from(raw).ok().and_then(|key| records.get(&key).copied())
}

pub fn reading_result_err(raw: i64) -> Option<String> {
    u32::try_from(raw).err().map(|error| error.to_string())
}

pub fn reading_drain_messages(receiver: std::sync::mpsc::Receiver<i32>) {
    while receiver.recv().is_ok() {}
}

pub mod reading_result_shadows {
    struct Result;

    impl Result {
        fn ok(self) -> Option<i32> { None }
        fn err(self) -> Option<String> { None }
    }

    pub fn reading_local_result() -> (Option<i32>, Option<String>) {
        (Result.ok(), Result.err())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_only_the_first_item_of_a_present_entry() {
        let encounter = Encounter {
            crew_index: HashMap::from([("captain".into(), (3, 2))]),
        };
        assert_eq!(prepare_crew(&encounter, "captain"), Some(3));
        assert_eq!(prepare_crew(&encounter, "missing"), None);
        assert_eq!(encounter.crew_index["captain"], (3, 2));
    }
}
