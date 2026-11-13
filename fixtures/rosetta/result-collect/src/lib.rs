use std::collections::HashMap;

pub fn assemble(ids: &[u8], views: &mut HashMap<u8, String>) -> Result<Vec<String>, String> {
    let values = ids
        .iter()
        .map(|id| views.remove(id).ok_or_else(|| format!("no view for {id}")))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(values)
}

pub fn typed<I: Iterator<Item = Option<u8>>>(items: I) -> Result<Vec<u8>, &'static str> {
    #[allow(unused_parens)]
    let output: std::result::Result<std::vec::Vec<u8>, &'static str> =
        (Iterator::collect(Iterator::map(items, |item| item.ok_or("missing"))));
    output
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn segment(&mut self) -> Result<&'a u8, &'static str> {
        let (head, rest) = self.0.split_first().ok_or("missing segment")?;
        self.0 = rest;
        Ok(head)
    }
}

pub fn segments(values: &[u8], count: usize) -> Result<Vec<&u8>, &'static str> {
    let mut reader = Reader(values);
    (0..count)
        .map(|_| reader.segment())
        .collect::<Result<Vec<_>, _>>()
}

pub fn assemble_values(values: &[u8]) -> Result<Vec<u8>, &'static str> {
    values
        .iter()
        .map(|&value| {
            let value = value.checked_add(1).ok_or("overflow")?;
            Ok(value)
        })
        .collect::<Result<Vec<_>, _>>()
}

struct Local;

impl FromIterator<Local> for Result<Vec<u8>, &'static str> {
    fn from_iter<I: IntoIterator<Item = Local>>(_: I) -> Self {
        Err("custom collector")
    }
}

pub fn custom_item(values: &[u8]) -> Result<Vec<u8>, &'static str> {
    values
        .iter()
        .map(|&value| {
            std::hint::black_box(Some(value).ok_or("missing")).unwrap_or(0);
            Local
        })
        .collect::<Result<Vec<_>, _>>()
}

pub fn custom_item_direct() -> Result<Vec<u8>, &'static str> {
    [Local].into_iter().collect::<Result<Vec<_>, _>>()
}

pub fn custom_item_by_type(values: &[u8]) -> Result<Vec<u8>, &'static str> {
    let output: Result<Vec<u8>, &'static str> = values.iter().map(|_| Local).collect();
    output
}

type Alias<T, E> = Result<T, E>;

pub fn alias(items: Vec<Result<u8, &'static str>>) -> Alias<Vec<u8>, &'static str> {
    items
        .into_iter()
        .map(|item| Ok(item?))
        .collect::<Alias<Vec<_>, _>>()
}

pub fn inferred(items: Vec<Result<u8, &'static str>>) -> Result<Vec<u8>, &'static str> {
    items
        .into_iter()
        .map(|item| Ok(item?))
        .collect::<Result<_, _>>()
}

pub fn separate_target(items: Vec<Result<u8, &'static str>>) -> Result<Vec<u8>, &'static str> {
    let output: Result<Vec<u8>, &'static str> =
        std::hint::black_box(items.into_iter().map(|item| Ok(item?)).collect());
    output
}

pub fn parallel(items: Vec<Result<u8, &'static str>>) -> Result<Vec<u8>, &'static str> {
    use rayon::prelude::*;
    items.into_par_iter().collect::<Result<Vec<_>, _>>()
}

pub mod shadowed {
    pub struct Result<T, E>(pub T, pub Option<E>);

    impl<T, E> FromIterator<std::result::Result<T, E>> for Result<Vec<T>, E> {
        fn from_iter<I: IntoIterator<Item = std::result::Result<T, E>>>(iter: I) -> Self {
            Self(
                iter.into_iter()
                    .filter_map(std::result::Result::ok)
                    .collect(),
                None,
            )
        }
    }

    pub fn custom_result(
        items: Vec<std::result::Result<u8, &'static str>>,
    ) -> Result<Vec<u8>, &'static str> {
        items
            .into_iter()
            .map(|item| std::result::Result::Ok(item?))
            .collect::<Result<Vec<_>, _>>()
    }
}

pub mod shadowed_vec {
    pub struct Vec<T>(pub Option<T>);

    impl<T> FromIterator<T> for Vec<T> {
        fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
            Self(iter.into_iter().next())
        }
    }

    pub fn custom_vec(
        items: std::vec::Vec<Result<u8, &'static str>>,
    ) -> Result<Vec<u8>, &'static str> {
        items
            .into_iter()
            .map(|item| Ok(item?))
            .collect::<Result<Vec<_>, _>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_error_stops_collection_and_keeps_previous_mutations() {
        let mut views = HashMap::from([(1, String::from("first")), (3, String::from("third"))]);
        assert_eq!(
            assemble(&[1, 2, 3], &mut views),
            Err(String::from("no view for 2"))
        );
        assert_eq!(views, HashMap::from([(3, String::from("third"))]));
        assert_eq!(assemble(&[], &mut views), Ok(vec![]));
        assert_eq!(custom_item(&[1, 2]), Err("custom collector"));
    }
}
