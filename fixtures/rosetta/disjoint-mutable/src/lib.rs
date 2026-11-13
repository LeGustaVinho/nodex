use std::slice::GetDisjointMutError;

pub fn collide(ships: &mut Vec<i32>, order: &[usize], i: usize, j: usize) {
    let [a, b] = ships
        .get_disjoint_mut([order[i], order[j]])
        .expect("distinct ships");
    *a += 1;
    *b -= 1;
}

pub fn intervals(slots: &mut [i32]) -> Result<[&mut [i32]; 2], GetDisjointMutError> {
    <[i32]>::get_disjoint_mut(slots, [3..4, 0..2])
}

pub struct Local;

impl Local {
    fn get_disjoint_mut(&mut self, _indices: [usize; 2]) -> i32 {
        0
    }
}

pub fn homonym() -> i32 {
    Local.get_disjoint_mut([0, 0])
}

#[test]
fn borrows_are_returned_in_input_order() {
    let mut ships = vec![0, 10, 20];
    collide(&mut ships, &[2, 0], 0, 1);
    assert_eq!(ships, [-1, 10, 21]);
    assert!(matches!(ships.get_disjoint_mut([2, 2]), Err(GetDisjointMutError::OverlappingIndices)));
    assert!(matches!(ships.get_disjoint_mut([3, 0]), Err(GetDisjointMutError::IndexOutOfBounds)));
}
