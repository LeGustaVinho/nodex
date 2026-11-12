use std::task::Poll;

pub fn search(values: &[Option<i32>]) -> Option<i32> {
    values.iter().find_map(|value| Some((*value)? + 1))
}

#[allow(unused_parens)]
pub fn search_ufcs(values: &[Option<i32>]) -> Option<i32> {
    Iterator::find_map(&mut values.iter(), (|value| Some((*value)? + 1)))
}

pub fn keep_values(values: &[Option<i32>]) -> Vec<i32> {
    values.iter().filter_map(|value| Some((*value)? + 1)).collect()
}

pub fn fold_option_ufcs(values: &[Option<i32>]) -> Option<i32> {
    Iterator::try_fold(&mut values.iter(), 0, |sum, value| Some(sum + (*value)?))
}

pub fn fold_result(values: &[Result<i32, &'static str>]) -> Result<i32, &'static str> {
    values.iter().try_fold(0, |sum, value| Ok(sum + (*value)?))
}

pub fn fold_poll(values: &[Result<i32, &'static str>]) -> Poll<Result<i32, String>> {
    values.iter().try_fold(Poll::Ready(0), |sum, value| {
        let Poll::Ready(sum) = sum else { return Poll::Pending };
        let value = (*value)?;
        Poll::Ready(Ok(sum + value))
    })
}

struct Local;

impl Local {
    fn find_map(&self, callback: impl FnOnce() -> Option<i32>) -> Option<i32> {
        callback().or(Some(42))
    }
}

pub fn local_method(value: Option<i32>) -> Option<i32> {
    Local.find_map(|| Some(value? + 1))
}

pub fn nested_closure(values: &[Option<i32>]) -> Option<i32> {
    values.iter().find_map(|value| (|| Some((*value)? + 1))())
}

pub fn nested_future(values: &[Option<i32>]) -> Option<impl Future<Output = Option<i32>> + '_> {
    values.iter().find_map(|value| Some(async move { Some((*value)?) }))
}

pub fn fold_initial_closure(values: &[i32], initial: Option<i32>) -> Option<impl FnOnce() -> Option<i32>> {
    values.iter().try_fold(move || Some(initial?), |accumulator, _| Some(accumulator))
}

pub fn outer_question_mark(values: &[Option<i32>]) -> Option<i32> {
    let found = values.iter().find_map(|value| Some((*value)? + 1))?;
    Some(found)
}
