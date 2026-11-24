use crate::provider::{ProjectionEntity, ProviderSnapshot, SemanticRelationship};
use crate::sink::{build, sync, SyncOptions};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[allow(clippy::too_many_arguments)]
fn entity(
    kg_id: &str,
    title: &str,
    entity_type: &str,
    summary: &str,
    body: &str,
    source_ids: &[&str],
    target_ids: &[&str],
    relationships: Vec<SemanticRelationship>,
) -> ProjectionEntity {
    ProjectionEntity {
        kg_id: kg_id.to_string(),
        title: title.to_string(),
        entity_type: entity_type.to_string(),
        summary: summary.to_string(),
        metadata: {
            let mut metadata = Map::new();
            if !body.is_empty() {
                metadata.insert("body".into(), Value::from(body));
            }
            metadata
        },
        source_ids: source_ids.iter().map(|id| id.to_string()).collect(),
        target_ids: target_ids.iter().map(|id| id.to_string()).collect(),
        relation: None,
        relationships,
        mentions: Vec::new(),
        body: body.to_string(),
        version: None,
        event_seq: None,
    }
}

fn relationship(
    source_id: &str,
    target_id: &str,
    relation_type: &str,
    edge_id: &str,
) -> SemanticRelationship {
    let mut properties = Map::new();
    properties.insert("edge_id".into(), Value::from(edge_id));
    SemanticRelationship {
        source_id: source_id.to_string(),
        target_id: target_id.to_string(),
        relation_type: relation_type.to_string(),
        properties,
    }
}

fn demo_entities() -> Vec<ProjectionEntity> {
    let project_id = "node:project:obsidian-sink-demo";
    let kg_id = "node:concept:in-memory-kg";
    let inbox_id = "node:workflow:streaming-inbox";
    let stable_id = "node:governance:stable-paths";
    let guide_id = "node:document:quickstart";

    vec![
        entity(
            project_id,
            "Obsidian Sink Demo",
            "project",
            "End-to-end demo project for dumping and streaming into Obsidian.",
            "This is the main demo project note. ^rebuild-loop",
            &[stable_id],
            &[kg_id, inbox_id],
            vec![
                relationship(
                    project_id,
                    kg_id,
                    "depends_on",
                    "edge:demo:project-depends-on-kg",
                ),
                relationship(
                    project_id,
                    kg_id,
                    "contradicts",
                    "edge:demo:project-contradicts-kg",
                ),
                relationship(project_id, inbox_id, "uses", "edge:demo:project-uses-inbox"),
            ],
        ),
        entity(
            kg_id,
            "In-Memory KG",
            "concept",
            "The Kogwistar engine runs in memory for the demo.",
            "",
            &[project_id],
            &[],
            Vec::new(),
        ),
        entity(
            inbox_id,
            "Streaming Inbox",
            "workflow",
            "New changes are appended to an inbox before projection.",
            "",
            &[project_id],
            &[],
            Vec::new(),
        ),
        entity(
            stable_id,
            "Stable Paths",
            "governance",
            "Stable filenames keep Obsidian links healthy across reruns.",
            "",
            &[],
            &[project_id],
            vec![relationship(
                stable_id,
                project_id,
                "supports",
                "edge:demo:stable-supports-project",
            )],
        ),
        entity(
            guide_id,
            "Quickstart",
            "document",
            "The shortest path from in-memory KG to an Obsidian vault.",
            "",
            &[],
            &[project_id],
            vec![relationship(
                guide_id,
                project_id,
                "documents",
                "edge:demo:guide-documents-project",
            )],
        ),
    ]
}

pub fn run_end_to_end_demo(vault_root: &Path) -> Result<Value> {
    let _ = fs::remove_dir_all(vault_root);
    fs::create_dir_all(vault_root)?;

    let entities = demo_entities();
    let full_snapshot = ProviderSnapshot {
        entities: entities.clone(),
        version: Some(1),
        event_seq: Some(1),
    };
    let full_dump = build(vault_root, &full_snapshot)?;

    let inbox_id = "node:workflow:streaming-inbox";
    let update_id = "node:workflow:incremental-updates";
    let mut stream_entities = entities;
    stream_entities.push(entity(
        update_id,
        "Incremental Updates",
        "workflow",
        "A new event stream entry materializes only the impacted files.",
        "",
        &[inbox_id],
        &[],
        vec![relationship(
            inbox_id,
            update_id,
            "emits",
            "edge:demo:inbox-emits-update",
        )],
    ));
    let stream_snapshot = ProviderSnapshot {
        entities: stream_entities,
        version: Some(2),
        event_seq: Some(2),
    };
    let stream_update = sync(
        vault_root,
        &stream_snapshot,
        &SyncOptions {
            changed_ids: [inbox_id.to_string(), update_id.to_string()]
                .into_iter()
                .collect(),
            deleted_ids: HashSet::new(),
            affected_titles: [
                "Streaming Inbox".to_string(),
                "Incremental Updates".to_string(),
            ]
            .into_iter()
            .collect(),
        },
    )?;

    Ok(json!({
        "vault": vault_root.display().to_string(),
        "full_dump": Value::Object(full_dump),
        "stream_update": Value::Object(stream_update),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    #[test]
    fn runs_in_memory_end_to_end_projection() {
        let root = std::env::temp_dir().join(format!("kogwistar-rust-demo-{}", std::process::id()));
        let payload = run_end_to_end_demo(&root).unwrap();
        assert_eq!(payload["full_dump"]["notes"].as_u64(), Some(5));
        assert_eq!(payload["stream_update"]["notes"].as_u64(), Some(6));
        assert!(root.join("Projects/Obsidian Sink Demo.md").exists());
        assert!(root.join("Workflows/Incremental Updates.md").exists());
        assert!(root.join("System/ledger.json").exists());
        let state: Value = serde_json::from_str(
            &fs::read_to_string(root.join("System/materialized_state.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(state["version"], 2);
        assert_eq!(state["event_seq"], 2);
        assert_eq!(state["entities"].as_array().map(Vec::len), Some(6));
        fs::remove_dir_all(&root).unwrap();
    }
}
