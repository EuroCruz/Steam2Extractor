pub mod blob;
pub mod chunk;
pub mod cp1252;
pub mod database;
pub mod depot;
pub mod dictbin;
pub mod filter;
pub mod keys;
pub mod keysfile;
pub mod keysource;
pub mod manifest;
pub mod regex;
pub mod sim;
pub mod source;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
