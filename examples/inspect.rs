//! Run with `cargo run --example inspect -- path/to/document.json`.
use std::{env, error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    // TODO: Accept stdin when no file path is given.
    let path = env::args().nth(1).ok_or("usage: inspect <path.json>")?;
    let input = fs::read(path)?;
    let value = strata_json::from_slice(&input)?;
    println!("{value:#?}");
    Ok(())
}
