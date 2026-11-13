pub struct Fit {
    pub count: usize,
}

pub fn run_fighter_phase(mut origin: Option<&mut Fit>) -> bool {
    if let Some(source) = origin.as_deref_mut() {
        source.count += 1;
    }
    if let Some(source) = Option::as_deref_mut(&mut origin) {
        source.count += 2;
    }
    origin.is_some()
}

pub fn text_view(text: &mut Option<String>) -> Option<&mut str> {
    text.as_deref_mut()
}

pub fn string_view(text: &mut Option<String>) -> Option<&mut String> {
    text.as_mut()
}

pub struct Local(String);
impl Local {
    pub fn as_deref_mut(&mut self) -> Option<&mut str> {
        Some(&mut self.0)
    }
}

pub fn shadowed(local: &mut Local) -> Option<&mut str> {
    local.as_deref_mut()
}
