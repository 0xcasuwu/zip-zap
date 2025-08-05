use anyhow::Result;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TEST").is_err() {
        println!("cargo:rustc-env=RUSTFLAGS=-Clink-arg=-Tmetashrew.ld");
    }
    Ok(())
}
