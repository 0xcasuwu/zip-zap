use anyhow::Result;
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::prelude::*;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}
