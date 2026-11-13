pub fn advance(fields: &mut Vec<i32>) {
    fields.retain_mut(|field| {
        if *field < 0 {
            return false;
        }
        *field += 1;
        true
    });
}

pub fn filter(fields: &mut Vec<i32>) {
    Vec::retain(fields, |field| {
        if *field == 0 {
            return true;
        }
        false
    });
}

pub fn forward(fields: &mut Vec<i32>, keep: impl FnMut(&i32) -> bool) {
    fields.retain(keep);
}

pub fn explicit_tail(fields: &mut Vec<i32>) {
    fields.retain(|_| {
        return false;
    });
}

pub fn nested(fields: &mut Vec<i32>) {
    fields.retain(|_| {
        let local = || {
            return true;
        };
        std::hint::black_box(local());
        let future = async {
            return true;
        };
        std::mem::drop(future);
        false
    });
}

pub fn map_control(map: &mut indexmap::IndexMap<u8, u8>) {
    map.retain(|_, _| {
        return false;
    });
}

pub struct Local;
impl Local {
    fn retain(&mut self, mut predicate: impl FnMut(&i32) -> bool) {
        std::hint::black_box(predicate(&0));
    }
}

pub fn local_control(value: &mut Local) {
    value.retain(|_| {
        return false;
    });
}
