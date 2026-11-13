use std::collections::HashMap;

pub fn js_to_fixed(x: f64) -> String {
    let mut text = format!("{:.2}", x.abs());
    if x < 0.0 {
        text.insert(0, '-');
    }
    text
}

pub fn insert_at(text: &mut String, index: usize, character: char) {
    text.insert(index, character);
}

pub fn qualified(text: &mut String) {
    String::insert(text, 0, '+');
}

pub fn insert_item(values: &mut Vec<u32>, position: usize, value: u32) {
    values.insert(position, value);
}

pub fn insert_key(values: &mut HashMap<u32, String>, key: u32, value: String) -> Option<String> {
    values.insert(key, value)
}

mod local {
    struct String;

    impl String {
        fn insert(&mut self, _: usize, _: char) {}
    }

    pub fn local_insert() {
        let mut text = String;
        text.insert(0, '-');
    }
}
