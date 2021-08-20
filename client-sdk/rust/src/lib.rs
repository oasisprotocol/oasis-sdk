#![deny(rust_2018_idioms, unreachable_pub)]
#![forbid(unsafe_code)]

mod client;
mod requests;
pub mod types;
pub mod wallet;

pub use tonic;
pub use tower;

pub use client::{Client, Error};
