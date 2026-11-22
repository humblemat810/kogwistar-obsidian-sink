use crate::provider::{iter_related_ids, short_id, ProjectionEntity, ProviderSnapshot};
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct SinkPaths;

impl SinkPaths {
    pub fn note_path(
        vault_root: &Path,
        entity: &ProjectionEntity,
        path_by_id: &HashMap<String, PathBuf>,
    ) -> PathBuf {
        path_by_id.get(&entity.kg_id).cloned().unwrap_or_else(|| {
            vault_root
                .join("Notes")
                .join(format!("{}.md", safe_title(&entity.title)))
        })
    }

    pub fn canvas_path(vault_root: &Path, entity: &ProjectionEntity) -> PathBuf {
        let mut digest = Sha1::new();
        digest.update(entity.kg_id.as_bytes());
        let digest = format!("{:x}", digest.finalize());
        vault_root.join("Views").join(format!(
            "{}--{}.canvas",
            safe_title(&entity.title),
            &digest[..10]
        ))
    }
}

pub fn safe_title(title: &str) -> String {
    let mut safe = title.split_whitespace().collect::<Vec<_>>().join(" ");
    safe = safe
        .chars()
        .map(|character| match character {
            '/' => '-',
            '\\' => '-',
            ':' => '-',
            '*' => '-',
            '?' => '-',
            '"' => '\'',
            '<' => '(',
            '>' => ')',
            '|' => '-',
            value => value,
        })
        .collect();
    let safe = safe.trim_end_matches([' ', '.']).to_string();
    if safe.is_empty() {
        "Untitled".to_string()
    } else {
        safe
    }
}

fn entity_dir(entity_type: &str) -> &'static str {
    match entity_type.to_lowercase().as_str() {
        "concept" => "Concepts",
        "document" => "Documents",
        "project" => "Projects",
        "person" => "People",
        "workflow" => "Workflows",
        "run" => "Runs",
        "conversation" => "Conversations",
        "governance" => "Governance",
        _ => "Notes",
    }
}

pub fn allocate_paths(
    vault_root: &Path,
    entities: &[ProjectionEntity],
) -> HashMap<String, PathBuf> {
    let mut reserved = std::collections::HashSet::new();
    let mut paths = HashMap::new();
    let mut grouped: HashMap<(String, String), Vec<&ProjectionEntity>> = HashMap::new();
    for entity in entities {
        grouped
            .entry((
                entity_dir(&entity.entity_type).to_string(),
                safe_title(&entity.title),
            ))
            .or_default()
            .push(entity);
    }
    let mut keys: Vec<_> = grouped.keys().cloned().collect();
    keys.sort();
    for key in keys {
        let mut group = grouped.remove(&key).expect("group exists");
        group.sort_by(|left, right| left.kg_id.cmp(&right.kg_id));
        let (folder, stem) = key;
        for (index, entity) in group.into_iter().enumerate() {
            let relative = if index == 0 && reserved.insert(format!("{folder}/{stem}.md")) {
                PathBuf::from(folder.clone()).join(format!("{stem}.md"))
            } else {
                let suffix = short_id(&entity.kg_id);
                PathBuf::from(folder.clone()).join(format!("{stem}__{suffix}.md"))
            };
            paths.insert(entity.kg_id.clone(), vault_root.join(relative));
        }
    }
    paths
}

fn wiki_target(
    path: &Path,
    vault_root: &Path,
    title: &str,
    title_counts: &HashMap<String, usize>,
) -> String {
    if title_counts.get(title).copied().unwrap_or(0) <= 1 {
        return title.to_string();
    }
    let relative = path.strip_prefix(vault_root).unwrap_or(path);
    relative
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/")
}

fn format_wikilink(target: &str, display: Option<&str>) -> String {
    if let Some(display) = display {
        if display != target
            && display
                != Path::new(target)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
        {
            return format!("[[{target}|{display}]]");
        }
    }
    format!("[[{target}]]")
}

pub fn render_note(
    entity: &ProjectionEntity,
    snapshot: &ProviderSnapshot,
    vault_root: &Path,
    path_by_id: &HashMap<String, PathBuf>,
) -> (String, usize) {
    let title_counts = entity
        .title
        .to_string()
        .split_whitespace()
        .map(|_| 0)
        .collect::<Vec<_>>();
    let _ = title_counts;
    let mut title_counts: HashMap<String, usize> = HashMap::new();
    for item in &snapshot.entities {
        *title_counts.entry(item.title.clone()).or_default() += 1;
    }
    let mut dangling = 0;
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!(
        "kg_id: {}",
        serde_json::to_string(&entity.kg_id).unwrap()
    ));
    lines.push(format!(
        "kg_type: {}",
        serde_json::to_string(&entity.entity_type).unwrap()
    ));
    lines.push("kg_projection: \"obsidian_markdown_v1\"".to_string());
    lines.push(format!(
        "kg_version: {}",
        entity
            .version
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    lines.push(format!(
        "kg_last_event_seq: {}",
        entity
            .event_seq
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    lines.push("kg_source_of_truth: \"kogwistar\"".to_string());
    lines.push("kg_edit_policy:".to_string());
    lines.push("  summary: mixed".to_string());
    lines.push("  notes: user_owned".to_string());
    lines.push("  structure: read_only".to_string());
    lines.push(format!(
        "title: {}",
        serde_json::to_string(&entity.title).unwrap()
    ));
    lines.push(format!(
        "tags: {}",
        serde_json::to_string(
            &entity
                .metadata
                .get("tags")
                .cloned()
                .unwrap_or(Value::Array(Vec::new()))
        )
        .unwrap()
    ));
    lines.push(format!(
        "aliases: {}",
        serde_json::to_string(
            &entity
                .metadata
                .get("aliases")
                .cloned()
                .unwrap_or(Value::Array(Vec::new()))
        )
        .unwrap()
    ));
    lines.push("---".to_string());
    lines.push(String::new());
    lines.push(format!("# {}", entity.title));
    lines.push(String::new());
    lines.push("## Summary".to_string());
    lines.push(entity.summary.clone());
    lines.push(String::new());
    lines.push("## Body".to_string());
    lines.push(entity.body.clone());
    lines.push(String::new());
    lines.push("## Links".to_string());
    let mut links = Vec::new();
    for source_id in &entity.source_ids {
        let Some(target) = path_by_id.get(source_id) else {
            dangling += 1;
            lines.push(format!("- source -> [[Notes/{}]]", safe_title(source_id)));
            continue;
        };
        let target_title = snapshot
            .entities
            .iter()
            .find(|candidate| candidate.kg_id == *source_id)
            .map(|candidate| candidate.title.clone())
            .unwrap_or_else(|| {
                target
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
                    .to_string()
            });
        links.push(format!(
            "- source -> {}",
            format_wikilink(
                &wiki_target(target, vault_root, &target_title, &title_counts),
                Some(&target_title)
            )
        ));
    }
    for target_id in &entity.target_ids {
        let Some(target) = path_by_id.get(target_id) else {
            dangling += 1;
            lines.push(format!("- target -> [[Notes/{}]]", safe_title(target_id)));
            continue;
        };
        let target_title = snapshot
            .entities
            .iter()
            .find(|candidate| candidate.kg_id == *target_id)
            .map(|candidate| candidate.title.clone())
            .unwrap_or_else(|| {
                target
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
                    .to_string()
            });
        links.push(format!(
            "- target -> {}",
            format_wikilink(
                &wiki_target(target, vault_root, &target_title, &title_counts),
                Some(&target_title)
            )
        ));
    }
    if entity.relation.is_some() {
        links.push(format!(
            "- relation: `{}`",
            entity.relation.clone().unwrap_or_default()
        ));
    }
    if links.is_empty() {
        lines.push("- none".to_string());
    } else {
        lines.extend(links);
    }
    lines.push(String::new());
    lines.push("## Provenance".to_string());
    if entity.mentions.is_empty() {
        lines.push("- none".to_string());
    } else {
        for mention in &entity.mentions {
            lines.push(format!(
                "- doc `{}` page {} chars {}:{} — {}",
                mention.doc_id,
                mention.page_number,
                mention.start_char,
                mention.end_char,
                mention.excerpt
            ));
        }
    }
    lines.push(String::new());
    lines.push("## User Notes".to_string());
    lines.push("<!-- USER-OWNED-START -->".to_string());
    lines.push(
        entity
            .metadata
            .get("user_notes")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    );
    lines.push("<!-- USER-OWNED-END -->".to_string());
    (lines.join("\n"), dangling)
}

pub fn render_canvas(entity: &ProjectionEntity, snapshot: &ProviderSnapshot) -> String {
    let mut nodes = vec![json!({
        "id": entity.kg_id,
        "type": "text",
        "text": entity.title,
        "x": 100,
        "y": 100,
        "width": 260,
        "height": 80,
    })];
    let mut edges = Vec::new();
    for (index, related_id) in iter_related_ids(snapshot, &entity.kg_id)
        .into_iter()
        .enumerate()
        .take(20)
    {
        nodes.push(json!({
            "id": related_id,
            "type": "text",
            "text": related_id,
            "x": 420,
            "y": 100 + (index as u64 + 1) * 120,
            "width": 260,
            "height": 80,
        }));
        edges.push(json!({
            "id": format!("edge-{}", index + 1),
            "fromNode": entity.kg_id,
            "toNode": related_id,
            "fromSide": "right",
            "toSide": "left",
        }));
    }
    serde_json::to_string_pretty(&json!({"nodes": nodes, "edges": edges})).unwrap()
}

pub fn write_if_changed(path: &Path, text: &str) -> Result<bool> {
    if path.exists() && fs::read_to_string(path)? == text {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    Ok(true)
}

pub fn build(vault_root: &Path, snapshot: &ProviderSnapshot) -> Result<Map<String, Value>> {
    if !snapshot
        .entities
        .iter()
        .all(|entity| !entity.kg_id.trim().is_empty())
    {
        bail!("entity kg_id must be non-empty");
    }
    let path_by_id = allocate_paths(vault_root, &snapshot.entities);
    let mut written = 0;
    let mut canvases = 0;
    let mut dangling_links = 0;
    for entity in &snapshot.entities {
        let note_path = SinkPaths::note_path(vault_root, entity, &path_by_id);
        let (note, dangling) = render_note(entity, snapshot, vault_root, &path_by_id);
        dangling_links += dangling;
        if write_if_changed(&note_path, &note)? {
            written += 1;
        }
        let canvas_path = SinkPaths::canvas_path(vault_root, entity);
        if write_if_changed(&canvas_path, &render_canvas(entity, snapshot))? {
            canvases += 1;
        }
    }
    let mut stats = Map::new();
    stats.insert("notes".to_string(), Value::from(written));
    stats.insert("canvases".to_string(), Value::from(canvases));
    stats.insert("dangling_links".to_string(), Value::from(dangling_links));
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::from_entity_projection;

    fn snapshot() -> ProviderSnapshot {
        let payload = serde_json::json!({
            "last_seq": 7,
            "entities": {
                "alpha": {"deleted": false, "seq": 5, "entity": {"id": "alpha", "label": "Alpha", "type": "concept"}},
                "beta": {"deleted": false, "seq": 6, "entity": {"id": "beta", "label": "Beta", "type": "concept"}}
            }
        });
        from_entity_projection(&serde_json::to_string(&payload).unwrap()).unwrap()
    }

    #[test]
    fn builds_notes_and_canvases() {
        let root = std::env::temp_dir().join(format!("kogwistar-sink-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let stats = build(&root, &snapshot()).unwrap();
        assert_eq!(stats.get("notes").and_then(Value::as_u64), Some(2));
        assert_eq!(stats.get("canvases").and_then(Value::as_u64), Some(2));
        assert!(root.join("Concepts/Alpha.md").exists());
        fs::remove_dir_all(&root).unwrap();
    }
}
