pub fn encoded_length(json: String) -> usize {
    json.len()
}

pub fn fractional_length(fraction: &str) -> usize {
    fraction.len()
}

pub fn json_items(items: &Vec<serde_json::Value>) -> usize {
    items.len()
}

pub fn slice_items(items: &[u32]) -> usize {
    items.len()
}

pub fn object_entries(entries: &serde_json::Map<String, serde_json::Value>) -> usize {
    entries.len()
}

pub struct Custom;
impl Custom {
    fn len(&self) -> usize { 7 }
}

pub fn local_length(value: Custom) -> usize { value.len() }
