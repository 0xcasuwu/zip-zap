use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use hex;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use std::process::{Command, Stdio};

fn compress(binary: Vec<u8>) -> Result<Vec<u8>> {
    let mut writer = GzEncoder::new(Vec::<u8>::with_capacity(binary.len()), Compression::best());
    writer.write_all(&binary)?;
    Ok(writer.finish()?)
}

fn build_alkane(
    alkane_dir: &Path,
    features: Vec<&'static str>,
) -> Result<()> {
    let mut command = Command::new("cargo");
    command
        .current_dir(alkane_dir)
        .arg("build")
        .arg("--release")
        .arg("--target=wasm32-unknown-unknown");

    if !features.is_empty() {
        command.arg("--features").arg(features.join(","));
    }

    let status = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to build alkane in {:?}",
            alkane_dir
        ))
    }
}

fn main() {
    let manifest_dir_string = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(&manifest_dir_string);
    let wasm_dir = manifest_dir.join("target");
    let write_dir = manifest_dir.join("src").join("precompiled");
    fs::create_dir_all(&write_dir).unwrap();
    let crates_dir = manifest_dir.join("alkanes");
    let mods = fs::read_dir(&crates_dir)
        .unwrap()
        .filter_map(|entry_res| {
            let entry = entry_res.ok()?;
            if entry.file_type().ok()?.is_dir() {
                let name = entry.file_name().into_string().ok()?;
                if name != "target" && name != "release" {
                    Some(name)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<String>>();
    let files = mods
        .clone()
        .into_iter()
        .filter_map(|name| {
            Some(name)
        })
        .collect::<Vec<String>>();
    files
        .into_iter()
        .map(|v| -> Result<String> {
            let alkane_path = crates_dir.join(&v);
            if let Err(e) = build_alkane(&alkane_path, vec![]) {
                eprintln!("Failed to build alkane {}: {}", v, e);
                return Err(e);
            }
            let subbed = v.replace("-", "_");
            eprintln!(
                "write: {}",
                write_dir
                    .join(subbed.clone() + "_build.rs")
                    .into_os_string()
                    .to_str()
                    .unwrap()
            );
            let wasm_path = wasm_dir
                .join("wasm32-unknown-unknown")
                .join("release")
                .join(subbed.clone() + ".wasm");
            if !wasm_path.exists() {
                return Err(anyhow::anyhow!("WASM file not found: {:?}", wasm_path));
            }
            let f: Vec<u8> = fs::read(&wasm_path)?;
            let compressed: Vec<u8> = compress(f.clone())?;
            fs::write(
                &wasm_dir
                    .join("wasm32-unknown-unknown")
                    .join("release")
                    .join(subbed.clone() + ".wasm.gz"),
                &compressed,
            )?;
            let data: String = hex::encode(&f);
            fs::write(
                &write_dir.join(subbed.clone() + "_build.rs"),
                String::from("use hex_lit::hex;\n#[allow(long_running_const_eval)]\npub fn get_bytes() -> Vec<u8> { (&hex!(\"")
                    + data.as_str()
                    + "\")).to_vec() }",
            )?;
            eprintln!(
                "build: {}",
                write_dir
                    .join(subbed.clone() + "_build.rs")
                    .into_os_string()
                    .to_str()
                    .unwrap()
            );
            Ok(subbed)
        })
        .collect::<Result<Vec<String>>>()
        .unwrap();
    eprintln!(
        "write test builds to: {}",
        write_dir
            .join("mod.rs")
            .into_os_string()
            .to_str()
            .unwrap()
    );
    fs::write(
        &write_dir.join("mod.rs"),
        mods.into_iter()
            .map(|v| v.replace("-", "_"))
            .fold(String::default(), |r, v| {
                r + "pub mod " + v.as_str() + "_build;\n"
            }),
    )
    .unwrap();
}
