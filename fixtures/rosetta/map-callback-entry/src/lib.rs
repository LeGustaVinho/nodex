pub fn json_lengths(items: &[(String, String)]) -> usize {
    items
        .iter()
        .map(|(key, item)| {
            key.chars()
                .map(|character| character.len_utf8())
                .sum::<usize>()
                + 1
                + item.len()
        })
        .sum()
}

pub fn iterator_ufcs(items: Vec<String>) -> Vec<usize> {
    Iterator::map(items.into_iter(), |word| {
        let len = word.len();
        len + 1
    })
    .collect()
}

pub fn option_method(item: Option<String>) -> Option<usize> {
    item.map(|item| {
        let len = item.len();
        len + 1
    })
}

pub fn option_ufcs(item: Option<String>) -> Option<usize> {
    Option::map(item, move |item| {
        let len = item.len();
        len + 1
    })
}

pub fn option_async(item: Option<String>) -> Option<impl Future<Output = usize>> {
    item.map(async |item| {
        let len = item.len();
        len + 1
    })
}

pub fn iterator_async(items: Vec<String>) -> impl Iterator<Item = impl Future<Output = usize>> {
    items.into_iter().map(async |item| {
        let len = item.len();
        len + 1
    })
}

pub fn result_map(item: Result<String, ()>) -> Result<usize, ()> {
    item.map(|item| {
        let len = item.len();
        len + 1
    })
}

pub struct Local;

impl Local {
    fn map(self, f: impl FnOnce(u8) -> u8) -> u8 {
        f(1)
    }
}

pub fn local_map(item: Local) -> u8 {
    item.map(|item| {
        let next = item + 1;
        next + 1
    })
}

pub fn named_mapper(item: Option<String>) -> Option<usize> {
    let measure = |item: String| {
        let len = item.len();
        len + 1
    };
    item.map(measure)
}

pub fn inline_mapper(item: Option<String>) -> Option<usize> {
    item.map(|item| item.len())
}

pub fn nested(items: Vec<Option<u8>>) -> Vec<Option<u8>> {
    items
        .into_iter()
        .map(|item| {
            let adjust = |value| {
                let next = value + 1;
                next + 1
            };
            item.map(|value| {
                let mapped = adjust(value);
                mapped + 1
            })
        })
        .collect()
}
