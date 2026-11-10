pub fn sum_of_even_squares(values: &[i32]) -> i32 {
    values
        .iter()
        .filter(|&&value| value % 2 == 0)
        .map(|value| value * value)
        .sum()
}

pub fn adjust_and_sum(mut values: Vec<i32>) -> i32 {
    for value in values.iter_mut() {
        *value += 1;
    }
    values.into_iter().sum()
}

pub fn first_positive(value: Option<i32>) -> Option<i32> {
    value.and_then(|item| (item > 0).then_some(item))
}

pub fn borrowed_sum(values: &Vec<i32>) -> i32 {
    values.into_iter().copied().sum()
}
