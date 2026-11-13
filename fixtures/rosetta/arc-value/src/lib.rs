use std::{collections::BTreeMap, rc::Rc, sync::Arc};

#[derive(Clone)]
pub struct Blueprint { pub cycle_ticks: f64 }

pub fn burst(mut bp: Arc<Blueprint>, cycle_ticks: f64) -> Arc<Blueprint> {
    if cycle_ticks.to_bits() != bp.cycle_ticks.to_bits() {
        Arc::make_mut(&mut bp).cycle_ticks = cycle_ticks;
    }
    bp
}

pub struct SharedMetadata(Arc<BTreeMap<u32, String>>);

impl SharedMetadata {
    pub fn entries_mut(&mut self) -> std::collections::btree_map::IterMut<'_, u32, String> {
        Arc::make_mut(&mut self.0).iter_mut()
    }

    pub fn into_entries(self) -> std::collections::btree_map::IntoIter<u32, String> {
        Arc::unwrap_or_clone(self.0).into_iter()
    }
}

pub fn keep_arc(value: &Arc<String>) -> Arc<String> { Arc::clone(value) }
pub fn get_mut(value: &mut Arc<String>) -> Option<&mut String> { Arc::get_mut(value) }
pub fn try_unwrap(value: Arc<String>) -> Result<String, Arc<String>> { Arc::try_unwrap(value) }
pub fn rc_mut(value: &mut Rc<String>) -> &mut String { Rc::make_mut(value) }
pub fn rc_owned(value: Rc<String>) -> String { Rc::unwrap_or_clone(value) }

mod local {
    pub struct Arc(String);

    impl Arc {
        fn make_mut(this: &mut Self) -> &mut String { &mut this.0 }
        fn unwrap_or_clone(this: Self) -> String { this.0 }
    }

    pub fn local_mut(value: &mut Arc) -> &mut String { Arc::make_mut(value) }
    pub fn local_owned(value: Arc) -> String { Arc::unwrap_or_clone(value) }
}
