use crate::provider::ProjectionEntity;
use crate::sink::{self, SyncOptions};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub struct EventOptions {
    pub from_seq: Option<u64>,
    pub to_seq: Option<u64>,
}

fn normalize_entity(
    value: &Value,
    event_seq: Option<u64>,
    version: Option<u64>,
) -> ProjectionEntity {
    let mut entity = crate::provider::projection_entity(value);
    if entity.event_seq.is_none() {
        entity.event_seq = event_seq;
    }
    if entity.version.is_none() {
        entity.version = version.or(entity.event_seq);
    }
    entity
}

pub fn consume_events(
    vault_root: &Path,
    events_path: &Path,
    options: &EventOptions,
) -> Result<Value> {
    let state_path = vault_root.join("System/materialized_state.json");
    let state: Value = fs::read_to_string(&state_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| json!({"version": null, "event_seq": null, "entities": []}));
    let mut entities_by_id: HashMap<String, ProjectionEntity> = HashMap::new();
    if let Some(Value::Array(values)) = state.get("entities") {
        for value in values {
            if let Ok(entity) = serde_json::from_value::<ProjectionEntity>(value.clone()) {
                entities_by_id.insert(entity.kg_id.clone(), entity);
            }
        }
    }
    let mut changed_ids = HashSet::new();
    let mut deleted_ids = HashSet::new();
    let mut affected_titles = HashSet::new();
    let mut last_event_seq = state.get("event_seq").and_then(Value::as_u64);
    let mut last_version = state.get("version").and_then(Value::as_u64);
    let mut inbox = String::new();
    for line in fs::read_to_string(events_path)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(line)?;
        let event_seq = event.get("event_seq").and_then(Value::as_u64);
        if let Some(from_seq) = options.from_seq {
            if event_seq.is_none() || event_seq < Some(from_seq) {
                continue;
            }
        }
        if let Some(to_seq) = options.to_seq {
            if event_seq.is_none() || event_seq > Some(to_seq) {
                continue;
            }
        }
        if event_seq.is_some() && last_event_seq.is_some() && event_seq <= last_event_seq {
            continue;
        }
        let entity_value = event.get("entity").cloned().unwrap_or(Value::Null);
        let entity_id = entity_value
            .get("id")
            .or_else(|| entity_value.get("kg_id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if entity_id.is_empty() {
            continue;
        }
        inbox.push_str(&format!("{}\n", serde_json::to_string(&event)?));
        match event.get("type").and_then(Value::as_str) {
            Some("entity.upsert") => {
                if let Some(previous) = entities_by_id.get(&entity_id) {
                    affected_titles.insert(previous.title.clone());
                }
                let version = event.get("version").and_then(Value::as_u64);
                let entity = normalize_entity(&entity_value, event_seq, version);
                affected_titles.insert(entity.title.clone());
                entities_by_id.insert(entity_id.clone(), entity);
                changed_ids.insert(entity_id);
            }
            Some("entity.delete") | Some("entity.remove") | Some("entity.tombstone") => {
                if let Some(previous) = entities_by_id.remove(&entity_id) {
                    affected_titles.insert(previous.title.clone());
                    deleted_ids.insert(entity_id);
                }
            }
            _ => continue,
        }
        if event_seq.is_some() {
            last_event_seq = event_seq;
        }
        if let Some(version) = event.get("version").and_then(Value::as_u64) {
            last_version = Some(version);
        }
    }
    let mut entities: Vec<ProjectionEntity> = entities_by_id.into_values().collect();
    entities.sort_by(|left, right| left.kg_id.cmp(&right.kg_id));
    let snapshot = crate::provider::ProviderSnapshot {
        entities: entities.clone(),
        version: last_version,
        event_seq: last_event_seq,
    };
    let stats = sink::sync(
        vault_root,
        &snapshot,
        &SyncOptions {
            changed_ids,
            deleted_ids,
            affected_titles,
        },
    )?;
    let inbox_path = vault_root.join("System/inbox.jsonl");
    if !inbox.is_empty() {
        let mut existing = fs::read_to_string(&inbox_path).unwrap_or_default();
        existing.push_str(&inbox);
        sink::write_atomic(&inbox_path, &existing)?;
    }
    Ok(Value::Object(stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn consumes_events() {
        let root = std::env::temp_dir().join(format!("kogwistar-event-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let events_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../examples/sample_events.jsonl");
        consume_events(
            &root,
            &events_path,
            &EventOptions {
                from_seq: None,
                to_seq: None,
            },
        )
        .unwrap();
        assert!(root.join("Concepts/Event Sourcing.md").exists());
        assert!(root.join("System/inbox.jsonl").exists());
        fs::remove_dir_all(&root).unwrap();
    }
}
