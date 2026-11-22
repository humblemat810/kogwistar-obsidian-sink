use crate::links::slugify;
use crate::provider::{iter_related_ids, short_id, ProjectionEntity, ProviderSnapshot};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionRecord {
    pub kg_id: String,
    pub file_path: String,
    pub projection_kind: String,
    pub canvas_path: Option<String>,
    pub title: Option<String>,
    pub last_projected_version: Option<u64>,
    pub last_applied_event_seq: Option<u64>,
    pub sync_mode: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default)]
    pub records: Map<String, Value>,
    #[serde(default)]
    pub by_id: Map<String, Value>,
}

#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    pub changed_ids: HashSet<String>,
    pub deleted_ids: HashSet<String>,
    pub affected_titles: HashSet<String>,
}

pub fn safe_title(title: &str) -> String {
    let normalized = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let mapped = normalized
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '|' => '-',
            '"' => '\'',
            '<' => '(',
            '>' => ')',
            other => other,
        })
        .collect::<String>()
        .trim_end_matches([' ', '.'])
        .to_string();
    if mapped.is_empty() {
        "Untitled".to_string()
    } else {
        mapped
    }
}

pub fn entity_dir(entity_type: &str) -> &'static str {
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
    ledger: &Ledger,
) -> HashMap<String, PathBuf> {
    let mut paths = HashMap::new();
    let mut reserved = HashSet::new();
    let mut grouped: HashMap<(String, String), Vec<&ProjectionEntity>> = HashMap::new();
    for entity in entities {
        if let Some(existing) = ledger.by_id.get(&entity.kg_id).and_then(Value::as_str) {
            let relative = PathBuf::from(existing);
            reserved.insert(relative.to_string_lossy().replace('\\', "/"));
            paths.insert(entity.kg_id.clone(), vault_root.join(relative));
        } else {
            grouped
                .entry((
                    entity_dir(&entity.entity_type).to_string(),
                    safe_title(&entity.title),
                ))
                .or_default()
                .push(entity);
        }
    }
    let mut keys: Vec<_> = grouped.keys().cloned().collect();
    keys.sort();
    for key in keys {
        let mut group = grouped.remove(&key).expect("group exists");
        group.sort_by(|left, right| left.kg_id.cmp(&right.kg_id));
        let (folder, stem) = key;
        let base = PathBuf::from(&folder).join(format!("{stem}.md"));
        let mut group_iter = group.into_iter();
        if !reserved.contains(&base.to_string_lossy().replace('\\', "/")) {
            if let Some(first) = group_iter.next() {
                paths.insert(first.kg_id.clone(), vault_root.join(&base));
                reserved.insert(base.to_string_lossy().replace('\\', "/"));
            }
        }
        for entity in group_iter {
            let mut suffix = short_id(&entity.kg_id);
            let mut candidate = PathBuf::from(&folder).join(format!("{stem}__{suffix}.md"));
            while reserved.contains(&candidate.to_string_lossy().replace('\\', "/")) {
                suffix = short_id(&format!("{}:{}", entity.kg_id, suffix));
                candidate = PathBuf::from(&folder).join(format!("{stem}__{suffix}.md"));
            }
            paths.insert(entity.kg_id.clone(), vault_root.join(&candidate));
            reserved.insert(candidate.to_string_lossy().replace('\\', "/"));
        }
    }
    paths
}

pub fn canvas_path(vault_root: &Path, entity: &ProjectionEntity) -> PathBuf {
    let mut digest = Sha256::new();
    digest.update(entity.kg_id.as_bytes());
    let digest = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    vault_root.join("Views").join(format!(
        "{}--{}.canvas",
        safe_title(&entity.title),
        &digest[..10]
    ))
}

pub fn write_atomic(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
    ));
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

pub fn write_if_changed(path: &Path, text: &str) -> Result<bool> {
    if path.exists() && fs::read_to_string(path)? == text {
        return Ok(false);
    }
    write_atomic(path, text)?;
    Ok(true)
}

pub fn load_ledger(path: &Path) -> Ledger {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn dump_json(path: &Path, value: &Value) -> Result<()> {
    write_atomic(path, &format!("{}\n", serde_json::to_string_pretty(value)?))
}

fn relative(path: &Path, vault_root: &Path) -> String {
    path.strip_prefix(vault_root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"))
}

fn title_counts(entities: &[ProjectionEntity]) -> HashMap<String, usize> {
    entities.iter().fold(HashMap::new(), |mut counts, entity| {
        *counts.entry(entity.title.clone()).or_default() += 1;
        counts
    })
}

fn format_wikilink(target: &str, display: Option<&str>) -> String {
    let filename = Path::new(target)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if let Some(display) = display {
        if !display.is_empty() && display != target && display != filename {
            return format!("[[{target}|{display}]]");
        }
    }
    format!("[[{target}]]")
}

fn fallback_link_target(target_id: &str) -> String {
    format!("Notes/{}", safe_title(target_id))
}

fn wiki_target(
    path: &Path,
    vault_root: &Path,
    title: &str,
    counts: &HashMap<String, usize>,
) -> String {
    if counts.get(title).copied().unwrap_or_default() <= 1 {
        return title.to_string();
    }
    let relative = relative(path, vault_root);
    Path::new(&relative)
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/")
}

fn entity_title(snapshot: &ProviderSnapshot, id: &str, fallback: &str) -> String {
    snapshot
        .entities
        .iter()
        .find(|entity| entity.kg_id == id)
        .map(|entity| entity.title.clone())
        .unwrap_or_else(|| fallback.to_string())
}

fn render_ref_link(
    reference: &Value,
    snapshot: &ProviderSnapshot,
    vault_root: &Path,
    paths: &HashMap<String, PathBuf>,
    counts: &HashMap<String, usize>,
    kind: &str,
) -> String {
    let target_id = reference
        .get("target_id")
        .or_else(|| reference.get("id"))
        .or_else(|| reference.get("kg_id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let title = entity_title(snapshot, &target_id, &target_id);
    let label = reference
        .get("label")
        .or_else(|| reference.get("text"))
        .and_then(Value::as_str)
        .unwrap_or(&title)
        .to_string();
    let mut fragment = reference
        .get("heading")
        .or_else(|| reference.get("block_id"))
        .or_else(|| reference.get("fragment"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let target_path = paths.get(&target_id);
    let mut target = match target_path {
        Some(path) => wiki_target(path, vault_root, &title, counts),
        None => fallback_link_target(&target_id),
    };
    if !fragment.is_empty() {
        if kind == "block" && !fragment.starts_with('^') {
            fragment = format!("^{fragment}");
        }
        target = format!("{target}#{fragment}");
    }
    let display = if label == title {
        None
    } else {
        Some(label.as_str())
    };
    format!("- {kind} -> {}", format_wikilink(&target, display))
}

fn render_attachment_link(reference: &Value) -> String {
    let path = if let Some(path) = reference.get("path") {
        path.as_str().unwrap_or_default().to_string()
    } else {
        reference
            .get("file")
            .or_else(|| reference.get("target"))
            .or_else(|| reference.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let label = reference
        .get("label")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&path)
                .to_string()
        });
    let display = if label == path {
        None
    } else {
        Some(label.as_str())
    };
    format!("- attachment -> {}", format_wikilink(&path, display))
}

pub fn render_note(
    entity: &ProjectionEntity,
    snapshot: &ProviderSnapshot,
    vault_root: &Path,
    paths: &HashMap<String, PathBuf>,
    event_seq: Option<u64>,
    version: Option<u64>,
) -> (String, usize) {
    let counts = title_counts(&snapshot.entities);
    let mut dangling = 0;
    let mut lines = vec![
        "---".to_string(),
        format!("kg_id: {}", serde_json::to_string(&entity.kg_id).unwrap()),
        format!(
            "kg_type: {}",
            serde_json::to_string(&entity.entity_type).unwrap()
        ),
        "kg_projection: \"obsidian_markdown_v1\"".to_string(),
        format!(
            "kg_version: {}",
            version
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        ),
        format!(
            "kg_last_event_seq: {}",
            event_seq
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        ),
        "kg_source_of_truth: \"kogwistar\"".to_string(),
        "kg_edit_policy:".to_string(),
        "  summary: mixed".to_string(),
        "  notes: user_owned".to_string(),
        "  structure: read_only".to_string(),
        format!("title: {}", serde_json::to_string(&entity.title).unwrap()),
        format!(
            "tags: {}",
            entity
                .metadata
                .get("tags")
                .cloned()
                .unwrap_or(Value::Array(Vec::new()))
                .to_string()
        ),
        format!(
            "aliases: {}",
            entity
                .metadata
                .get("aliases")
                .cloned()
                .unwrap_or(Value::Array(Vec::new()))
                .to_string()
        ),
        "---".to_string(),
        String::new(),
        format!("# {}", entity.title),
        String::new(),
        "## Summary".to_string(),
        entity.summary.clone(),
        String::new(),
        "## Body".to_string(),
        entity.body.clone(),
        String::new(),
        "## Links".to_string(),
    ];
    let mut links = Vec::new();
    for id in &entity.source_ids {
        let title = entity_title(snapshot, id, id);
        let target = paths
            .get(id)
            .map(|path| wiki_target(path, vault_root, &title, &counts))
            .unwrap_or_else(|| fallback_link_target(id));
        links.push(format!(
            "- source -> {}",
            format_wikilink(&target, Some(&title))
        ));
        if !paths.contains_key(id) {
            dangling += 1;
        }
    }
    for id in &entity.target_ids {
        let title = entity_title(snapshot, id, id);
        let target = paths
            .get(id)
            .map(|path| wiki_target(path, vault_root, &title, &counts))
            .unwrap_or_else(|| fallback_link_target(id));
        links.push(format!(
            "- target -> {}",
            format_wikilink(&target, Some(&title))
        ));
        if !paths.contains_key(id) {
            dangling += 1;
        }
    }
    if let Some(relation) = &entity.relation {
        links.push(format!("- relation: `{relation}`"));
    }
    if links.is_empty() {
        lines.push("- none".to_string());
    } else {
        lines.extend(links);
    }
    if !entity.relationships.is_empty() {
        lines.push(String::new());
        lines.push("## Semantic Relationships".to_string());
        for relationship in &entity.relationships {
            let source_title =
                entity_title(snapshot, &relationship.source_id, &relationship.source_id);
            let target_title =
                entity_title(snapshot, &relationship.target_id, &relationship.target_id);
            let source = paths
                .get(&relationship.source_id)
                .map(|path| wiki_target(path, vault_root, &source_title, &counts))
                .unwrap_or_else(|| fallback_link_target(&relationship.source_id));
            let target = paths
                .get(&relationship.target_id)
                .map(|path| wiki_target(path, vault_root, &target_title, &counts))
                .unwrap_or_else(|| fallback_link_target(&relationship.target_id));
            let mut line = format!("- {}:", relationship.relation_type);
            if !relationship.source_id.is_empty() && !relationship.target_id.is_empty() {
                line.push_str(&format!(
                    " {} -> {}",
                    format_wikilink(&source, Some(&source_title)),
                    format_wikilink(&target, Some(&target_title))
                ));
            } else if !relationship.source_id.is_empty() {
                line.push_str(&format!(
                    " {}",
                    format_wikilink(&source, Some(&source_title))
                ));
            } else if !relationship.target_id.is_empty() {
                line.push_str(&format!(
                    " {}",
                    format_wikilink(&target, Some(&target_title))
                ));
            }
            if !relationship.properties.is_empty() {
                line.push_str(&format!(
                    " | properties: {}",
                    serde_json::to_string(&Value::Object(relationship.properties.clone()))
                        .unwrap_or_default()
                ));
            }
            if !relationship.source_id.is_empty() && !paths.contains_key(&relationship.source_id) {
                dangling += 1;
            }
            if !relationship.target_id.is_empty() && !paths.contains_key(&relationship.target_id) {
                dangling += 1;
            }
            lines.push(line);
        }
    }
    for (kind, key) in [("heading", "heading_refs"), ("block", "block_refs")] {
        if let Some(Value::Array(references)) = entity.metadata.get(key) {
            if !references.is_empty() {
                lines.push(String::new());
                lines.push(format!("## {} References", kind.to_uppercase()));
                for reference in references {
                    lines.push(render_ref_link(
                        reference, snapshot, vault_root, paths, &counts, kind,
                    ));
                }
            }
        }
    }
    if let Some(Value::Array(attachments)) = entity.metadata.get("attachments") {
        if !attachments.is_empty() {
            lines.push(String::new());
            lines.push("## Attachments".to_string());
            for attachment in attachments {
                lines.push(render_attachment_link(attachment));
            }
        }
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
    lines.push(String::new());
    lines.push("```kogwistar-meta".to_string());
    lines.push("rendered_from: obsidian_sink_v1".to_string());
    lines.push(format!("entity_type: {}", entity.entity_type));
    let relation_handles = entity
        .source_ids
        .iter()
        .chain(entity.target_ids.iter())
        .cloned()
        .collect::<Vec<_>>();
    lines.push(format!(
        "relation_handles: {}",
        serde_json::to_string(&relation_handles).unwrap()
    ));
    lines.push("editable_sections:".to_string());
    lines.push("  - User Notes".to_string());
    lines.push("```".to_string());
    (lines.join("\n"), dangling)
}

pub fn render_canvas(entity: &ProjectionEntity, snapshot: &ProviderSnapshot) -> String {
    let mut related = iter_related_ids(snapshot, &entity.kg_id);
    related.dedup();
    let mut nodes = vec![json!({
        "id": slugify(&entity.kg_id),
        "type": "text",
        "text": entity.title,
        "x": 100,
        "y": 100,
        "width": 260,
        "height": 80,
    })];
    let mut edges = Vec::new();
    for (index, related_id) in related.iter().enumerate() {
        let node_id = slugify(related_id);
        nodes.push(json!({
            "id": node_id,
            "type": "text",
            "text": related_id,
            "x": 420,
            "y": 100 + (index as u64 + 1) * 120,
            "width": 260,
            "height": 80,
        }));
        edges.push(json!({
            "id": format!("edge-{}", index + 1),
            "fromNode": slugify(&entity.kg_id),
            "toNode": node_id,
            "fromSide": "right",
            "toSide": "left",
        }));
    }
    serde_json::to_string_pretty(&json!({"nodes": nodes, "edges": edges})).unwrap()
}

pub fn render_index(
    entities: &[ProjectionEntity],
    vault_root: &Path,
    paths: &HashMap<String, PathBuf>,
    version: Option<u64>,
    event_seq: Option<u64>,
) -> String {
    let counts = title_counts(entities);
    let mut sorted = entities.to_vec();
    sorted.sort_by(|left, right| {
        (&left.entity_type, left.title.to_lowercase())
            .cmp(&(&right.entity_type, right.title.to_lowercase()))
    });
    let mut lines = vec![
        "# Kogwistar Obsidian Sink Index".to_string(),
        String::new(),
        format!(
            "- version: `{}`",
            version
                .map(|value| value.to_string())
                .unwrap_or_else(|| "None".to_string())
        ),
        format!(
            "- event_seq: `{}`",
            event_seq
                .map(|value| value.to_string())
                .unwrap_or_else(|| "None".to_string())
        ),
        String::new(),
        "## Notes".to_string(),
    ];
    for entity in sorted {
        let Some(path) = paths.get(&entity.kg_id) else {
            continue;
        };
        let target = wiki_target(path, vault_root, &entity.title, &counts);
        lines.push(format!(
            "- {} ({})",
            format_wikilink(&target, Some(&entity.title)),
            entity.entity_type
        ));
    }
    format!("{}\n", lines.join("\n"))
}

fn write_materialized_state(
    path: &Path,
    snapshot: &ProviderSnapshot,
    entities: &[ProjectionEntity],
) -> Result<()> {
    dump_json(
        path,
        &json!({
            "version": snapshot.version,
            "event_seq": snapshot.event_seq,
            "entities": entities.iter().map(|entity| serde_json::to_value(entity).unwrap()).collect::<Vec<_>>(),
        }),
    )
}

fn entity_reference_ids(entity: &ProjectionEntity) -> HashSet<String> {
    let mut references = entity.source_ids.iter().cloned().collect::<HashSet<_>>();
    references.extend(entity.target_ids.iter().cloned());
    for key in ["heading_refs", "block_refs"] {
        if let Some(Value::Array(values)) = entity.metadata.get(key) {
            for value in values {
                if let Some(id) = value
                    .get("target_id")
                    .or_else(|| value.get("id"))
                    .or_else(|| value.get("kg_id"))
                    .and_then(Value::as_str)
                {
                    references.insert(id.to_string());
                }
            }
        }
    }
    for relationship in &entity.relationships {
        if !relationship.source_id.is_empty() {
            references.insert(relationship.source_id.clone());
        }
        if !relationship.target_id.is_empty() {
            references.insert(relationship.target_id.clone());
        }
    }
    references
}

fn compute_impacted_ids(entities: &[ProjectionEntity], options: &SyncOptions) -> HashSet<String> {
    let mut title_to_ids: HashMap<&str, HashSet<String>> = HashMap::new();
    for entity in entities {
        title_to_ids
            .entry(entity.title.as_str())
            .or_default()
            .insert(entity.kg_id.clone());
    }
    let mut impacted = options.changed_ids.clone();
    impacted.extend(options.deleted_ids.iter().cloned());
    for title in &options.affected_titles {
        if let Some(ids) = title_to_ids.get(title.as_str()) {
            impacted.extend(ids.iter().cloned());
        }
    }
    let seeds = impacted.clone();
    for entity in entities {
        if entity_reference_ids(entity)
            .intersection(&seeds)
            .next()
            .is_some()
        {
            impacted.insert(entity.kg_id.clone());
        }
    }
    impacted
}

fn effective_value(
    entity: &ProjectionEntity,
    snapshot: &ProviderSnapshot,
    field: &str,
) -> Option<u64> {
    match field {
        "event_seq" => entity.event_seq.or(snapshot.event_seq),
        "version" => entity.version.or(snapshot.version),
        _ => None,
    }
}

fn remove_entity(vault_root: &Path, ledger: &mut Ledger, entity_id: &str) -> (bool, bool) {
    let Some(Value::String(path_value)) = ledger.by_id.remove(entity_id) else {
        return (false, false);
    };
    let Some(record) = ledger.records.remove(&path_value) else {
        return (false, false);
    };
    let mut deleted_canvas = false;
    if let Some(Value::String(canvas)) = record.get("canvas_path") {
        if !ledger
            .records
            .values()
            .any(|other| other.get("canvas_path") == Some(&Value::String(canvas.clone())))
        {
            let path = vault_root.join(canvas);
            if path.exists() {
                fs::remove_file(path).ok();
                deleted_canvas = true;
            }
        }
    }
    let note_path = vault_root.join(&path_value);
    if note_path.exists() {
        fs::remove_file(note_path).ok();
    }
    (true, deleted_canvas)
}

pub fn build(vault_root: &Path, snapshot: &ProviderSnapshot) -> Result<Map<String, Value>> {
    if snapshot
        .entities
        .iter()
        .any(|entity| entity.kg_id.trim().is_empty())
    {
        bail!("entity kg_id must be non-empty");
    }
    let ledger_path = vault_root.join("System/ledger.json");
    let mut ledger = load_ledger(&ledger_path);
    let paths = allocate_paths(vault_root, &snapshot.entities, &ledger);
    let mut written = 0;
    let mut canvases = 0;
    let mut dangling_links = 0;
    for entity in &snapshot.entities {
        let path = paths.get(&entity.kg_id).expect("path exists");
        let event_seq = effective_value(entity, snapshot, "event_seq");
        let version = effective_value(entity, snapshot, "version");
        let (note, dangling) =
            render_note(entity, snapshot, vault_root, &paths, event_seq, version);
        dangling_links += dangling;
        write_atomic(path, &note)?;
        let canvas = canvas_path(vault_root, entity);
        write_atomic(&canvas, &render_canvas(entity, snapshot))?;
        written += 1;
        canvases += 1;
        let record = ProjectionRecord {
            kg_id: entity.kg_id.clone(),
            file_path: relative(path, vault_root),
            projection_kind: "note".to_string(),
            canvas_path: Some(relative(&canvas, vault_root)),
            title: Some(entity.title.clone()),
            last_projected_version: version,
            last_applied_event_seq: event_seq,
            sync_mode: "round_trip_safe_notes".to_string(),
        };
        ledger
            .records
            .insert(record.file_path.clone(), serde_json::to_value(&record)?);
        ledger.by_id.insert(
            entity.kg_id.clone(),
            Value::String(record.file_path.clone()),
        );
    }
    let system = vault_root.join("System");
    write_atomic(
        &system.join("index.md"),
        &render_index(
            &snapshot.entities,
            vault_root,
            &paths,
            snapshot.version,
            snapshot.event_seq,
        ),
    )?;
    dump_json(&ledger_path, &serde_json::to_value(&ledger)?)?;
    write_materialized_state(
        &system.join("materialized_state.json"),
        snapshot,
        &snapshot.entities,
    )?;
    let mut stats = Map::new();
    stats.insert("notes".into(), Value::from(written));
    stats.insert("canvases".into(), Value::from(canvases));
    stats.insert("dangling_links".into(), Value::from(dangling_links));
    Ok(stats)
}

pub fn sync(
    vault_root: &Path,
    snapshot: &ProviderSnapshot,
    options: &SyncOptions,
) -> Result<Map<String, Value>> {
    let ledger_path = vault_root.join("System/ledger.json");
    let mut ledger = load_ledger(&ledger_path);
    for entity_id in &options.deleted_ids {
        remove_entity(vault_root, &mut ledger, entity_id);
    }
    let impacted = compute_impacted_ids(&snapshot.entities, options);
    let paths = allocate_paths(vault_root, &snapshot.entities, &ledger);
    let mut written = 0;
    let mut canvases = 0;
    let deleted_canvases = 0;
    let mut dangling_links = 0;
    for entity in &snapshot.entities {
        if !impacted.contains(&entity.kg_id) {
            continue;
        }
        let path = paths.get(&entity.kg_id).expect("path exists");
        let event_seq = effective_value(entity, snapshot, "event_seq");
        let version = effective_value(entity, snapshot, "version");
        let (note, dangling) =
            render_note(entity, snapshot, vault_root, &paths, event_seq, version);
        dangling_links += dangling;
        if write_if_changed(path, &note)? {
            written += 1;
        }
        let canvas = canvas_path(vault_root, entity);
        if write_if_changed(&canvas, &render_canvas(entity, snapshot))? {
            canvases += 1;
        }
        let relative_path = relative(path, vault_root);
        let record = ProjectionRecord {
            kg_id: entity.kg_id.clone(),
            file_path: relative_path.clone(),
            projection_kind: "note".to_string(),
            canvas_path: Some(relative(&canvas, vault_root)),
            title: Some(entity.title.clone()),
            last_projected_version: version,
            last_applied_event_seq: event_seq,
            sync_mode: "streaming_incremental".to_string(),
        };
        ledger
            .records
            .insert(relative_path.clone(), serde_json::to_value(&record)?);
        ledger
            .by_id
            .insert(entity.kg_id.clone(), Value::String(relative_path));
    }
    let system = vault_root.join("System");
    write_if_changed(
        &system.join("index.md"),
        &render_index(
            &snapshot.entities,
            vault_root,
            &paths,
            snapshot.version,
            snapshot.event_seq,
        ),
    )?;
    dump_json(&ledger_path, &serde_json::to_value(&ledger)?)?;
    write_materialized_state(
        &system.join("materialized_state.json"),
        snapshot,
        &snapshot.entities,
    )?;
    let mut stats = Map::new();
    stats.insert("notes".into(), Value::from(snapshot.entities.len()));
    stats.insert("canvases".into(), Value::from(snapshot.entities.len()));
    stats.insert("updated_notes".into(), Value::from(written));
    stats.insert("updated_canvases".into(), Value::from(canvases));
    stats.insert("deleted_notes".into(), Value::from(0));
    stats.insert("deleted_canvases".into(), Value::from(deleted_canvases));
    stats.insert("dangling_links".into(), Value::from(dangling_links));
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::from_export_file;

    fn sample() -> ProviderSnapshot {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../examples/sample_graph_export.json");
        from_export_file(&path).unwrap()
    }

    #[test]
    fn builds_notes_ledger_and_canvases() {
        let root =
            std::env::temp_dir().join(format!("kogwistar-sink-build-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let stats = build(&root, &sample()).unwrap();
        assert_eq!(stats["notes"].as_u64(), Some(3));
        assert_eq!(stats["canvases"].as_u64(), Some(3));
        assert!(root.join("Concepts/Hypergraph RAG.md").exists());
        assert!(root.join("System/ledger.json").exists());
        assert!(root.join("System/index.md").exists());
        let ledger: Ledger =
            serde_json::from_str(&fs::read_to_string(root.join("System/ledger.json")).unwrap())
                .unwrap();
        assert_eq!(
            ledger
                .by_id
                .get("node:concept:hypergraph-rag")
                .and_then(Value::as_str),
            Some("Concepts/Hypergraph RAG.md")
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn syncs_incrementally() {
        let root = std::env::temp_dir().join(format!("kogwistar-sink-sync-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        build(&root, &sample()).unwrap();
        let mut snapshot = sample();
        snapshot.entities[0].title = "Renamed".to_string();
        let options = SyncOptions {
            changed_ids: ["node:concept:hypergraph-rag".to_string()]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let stats = sync(&root, &snapshot, &options).unwrap();
        assert_eq!(stats["updated_notes"].as_u64().unwrap(), 3);
        fs::remove_dir_all(&root).unwrap();
    }
}
