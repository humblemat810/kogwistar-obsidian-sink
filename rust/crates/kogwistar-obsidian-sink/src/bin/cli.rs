use anyhow::{bail, Result};
use kogwistar_obsidian_sink::provider::{from_entity_projection, from_export_file};
use kogwistar_obsidian_sink::sink::build;
use serde_json::Value;
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!("usage: kogwistar-obsidian-sink <build-from-export|build-from-projection> --input <path> --vault <path>");
    std::process::exit(2);
}

fn main() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.is_empty() {
        usage();
    }
    let command = arguments[0].as_str();
    let mut input = None;
    let mut vault = None;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--input" | "--sample" => {
                input = Some(PathBuf::from(
                    arguments
                        .get(index + 1)
                        .ok_or_else(|| anyhow::anyhow!("--input requires a path"))?,
                ));
                index += 2;
            }
            "--vault" => {
                vault = Some(PathBuf::from(
                    arguments
                        .get(index + 1)
                        .ok_or_else(|| anyhow::anyhow!("--vault requires a path"))?,
                ));
                index += 2;
            }
            _ => bail!("unknown argument: {}", arguments[index]),
        }
    }
    let Some(input) = input else {
        usage();
    };
    let Some(vault) = vault else {
        usage();
    };
    let snapshot = match command {
        "build-from-export" => from_export_file(&input)?,
        "build-from-projection" => {
            let raw = std::fs::read_to_string(&input)?;
            from_entity_projection(&raw)?
        }
        _ => usage(),
    };
    let stats = build(&vault, &snapshot)?;
    let stats: Value = Value::Object(stats);
    println!("{}", serde_json::to_string_pretty(&stats)?);
    Ok(())
}
