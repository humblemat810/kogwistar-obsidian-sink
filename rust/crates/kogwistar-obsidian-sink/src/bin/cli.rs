use anyhow::{bail, Result};
use kogwistar_obsidian_sink::event::{consume_events, EventOptions};
use kogwistar_obsidian_sink::provider::{from_entity_projection, from_export_file};
use kogwistar_obsidian_sink::roundtrip::inspect_note;
use kogwistar_obsidian_sink::sink::build;
use serde_json::Value;
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!("usage: kogwistar-obsidian-sink <build-from-export|build-from-projection> --input <path> --vault <path>");
    std::process::exit(2);
}

fn parse_event_options(arguments: &[String], start: usize) -> Result<EventOptions> {
    let mut from_seq = None;
    let mut to_seq = None;
    let mut index = start;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--from-seq" => {
                from_seq = Some(
                    arguments
                        .get(index + 1)
                        .ok_or_else(|| anyhow::anyhow!("--from-seq requires a value"))?
                        .parse()?,
                );
                index += 2;
            }
            "--to-seq" => {
                to_seq = Some(
                    arguments
                        .get(index + 1)
                        .ok_or_else(|| anyhow::anyhow!("--to-seq requires a value"))?
                        .parse()?,
                );
                index += 2;
            }
            _ => bail!("unknown argument: {}", arguments[index]),
        }
    }
    Ok(EventOptions { from_seq, to_seq })
}

fn main() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.is_empty() {
        usage();
    }
    let command = arguments[0].as_str();
    if command == "consume-events" {
        if arguments.len() < 5 || arguments[1] != "--events" || arguments[3] != "--vault" {
            usage();
        }
        let options = parse_event_options(&arguments, 5)?;
        let stats = consume_events(
            &PathBuf::from(&arguments[4]),
            &PathBuf::from(&arguments[2]),
            &options,
        )?;
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }
    if command == "inspect-note" {
        if arguments.len() != 3 || arguments[1] != "--path" {
            usage();
        }
        let value = inspect_note(&PathBuf::from(&arguments[2]))?;
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
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
