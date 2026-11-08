//! A Rust enum field driven by `match` and `if`, and a type-state door.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerState {
    Idle,
    Running,
    Jumping,
    Dead,
}

pub struct Player {
    state: PlayerState,
    lives: u32,
}

impl Player {
    pub fn new() -> Self {
        Player { state: PlayerState::Idle, lives: 3 }
    }

    pub fn run(&mut self) {
        if self.state == PlayerState::Idle {
            self.state = PlayerState::Running;
        }
    }

    pub fn jump(&mut self) {
        match self.state {
            PlayerState::Idle | PlayerState::Running => self.state = PlayerState::Jumping,
            _ => {}
        }
    }

    pub fn land(&mut self) {
        if self.state == PlayerState::Jumping {
            self.state = PlayerState::Idle;
        }
    }

    pub fn hit(&mut self) {
        self.lives -= 1;
        if self.lives == 0 {
            self.state = PlayerState::Dead;
        }
    }
}

pub struct Closed;
pub struct Open;

/// A door whose state is its type.
pub struct Door<S> {
    _state: std::marker::PhantomData<S>,
}

impl Door<Closed> {
    pub fn open(self) -> Door<Open> {
        Door { _state: std::marker::PhantomData }
    }
}

impl Door<Open> {
    pub fn close(self) -> Door<Closed> {
        Door { _state: std::marker::PhantomData }
    }
}
