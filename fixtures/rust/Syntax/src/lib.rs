//! Rust syntax coverage for Nodex: every construct here must lower without Opaque nodes.
#![allow(dead_code, unused_variables, unused_mut, unused_assignments, clippy::all)]

pub mod asyncs;
pub mod closures;
pub mod expressions;
pub mod impls;
pub mod items;
pub mod macros;
pub mod modern;
pub mod ownership;
pub mod patterns;
pub mod statements;
pub mod traits;

mod nested {
    pub mod deeper {
        pub fn answer() -> u32 {
            42
        }
    }

    pub use self::deeper::answer;
}

pub use nested::answer;
