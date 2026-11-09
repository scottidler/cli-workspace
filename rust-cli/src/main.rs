#![cfg_attr(
    debug_assertions,
    allow(unused_imports, unused_variables, unused_mut, dead_code, unused_assignments)
)]

use clap::Parser;
use load_config_derive::LoadConfig;
use serde::{Deserialize, Serialize};

#[derive(Parser, Deserialize, Serialize, Debug, LoadConfig)]
#[command(name = "rust-cli", version = env!("BUILD_VERSION"), about = "Example CLI with config loading")]
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
    // load_config parses the derived ConfigLoaderOpts, which carries no version, so answer it here.
    if std::env::args().skip(1).any(|arg| arg == "--version" || arg == "-V") {
        println!("rust-cli {}", env!("BUILD_VERSION"));
        return Ok(());
    }
    let opts = Opts::load_config()?;
    println!("opts={opts:?}");
    Ok(())
}
