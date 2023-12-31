#![cfg_attr(
    debug_assertions,
    allow(unused_imports, unused_variables, unused_mut, dead_code, unused_assignments)
)]

use clap::Parser;
use load_config_derive::LoadConfig;
use serde::{Deserialize, Serialize};

#[derive(Parser, Deserialize, Serialize, Debug, LoadConfig)]
struct Opts {
    #[clap(short, long, default_value = "config.yml")]
    config: String,

    #[clap(short, long, default_value = "John")]
    first_name: String,

    #[clap(short, long, default_value = "Doe")]
    last_name: String,

    #[clap(short, long, default_value = "42")]
    age: u8,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = Opts::load_config()?;
    println!("opts={opts:?}");
    Ok(())
}
