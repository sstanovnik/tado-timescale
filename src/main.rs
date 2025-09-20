extern crate core;

pub mod models {
    pub mod tado;
}

pub mod client;
pub mod config;
pub mod db {
    pub mod models;
}
pub mod schema;
pub mod utils;

fn main() {
    println!("Hello, world!");
}
