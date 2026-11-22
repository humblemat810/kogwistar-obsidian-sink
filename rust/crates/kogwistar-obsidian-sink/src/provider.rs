use anyhow::{bail, Result};
use serde_json::{Map, Value};
use sha1::{Digest, Sha1};
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectionEntity {
    pub kg_id: String,
    pub title: String,
    pub entity_type: String,
    pub summary: String,
    pub metadata: Map<String, Value>,
    pub source_ids: Vec<String>,
    pub target_ids: Vec<String>,
    pub relation: Option<String>,
    pub relationships: Vec<SemanticRelationship>,
    pub mentions: Vec<MentionSpan>,
    pub body: String,
    pub version: Option<u64>,
    pub event_seq: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MentionSpan {
    pub doc_id: String,
    pub page_number: u64,
    pub start_char: u64,
    pub end_char: u64,
    pub excerpt: String,
    pub document_page_url: String,
    pub context_before: String,
    pub context_after: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SemanticRelationship {
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String,
    pub properties: Map<String, Value>,
}

#[derive(Debug, Clone)]
pub struct ProviderSnapshot {
    pub entities: Vec<ProjectionEntity>,
    pub version: Option<u64>,
    pub event_seq: Option<u64>,
}

fn string(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| values.iter().map(|value| string(Some(value))).collect())
        .unwrap_or_default()
}

fn object(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn number(value: Option<&Value>) -> Option<u64> {
    value.and_then(Value::as_u64)
}

fn mention_span(value: &Value) -> MentionSpan {
    MentionSpan {
        doc_id: string(value.get("doc_id")),
        page_number: number(value.get("page_number")).unwrap_or(1),
        start_char: number(value.get("start_char")).unwrap_or_default(),
        end_char: number(value.get("end_char")).unwrap_or_default(),
        excerpt: string(value.get("excerpt")),
        document_page_url: string(value.get("document_page_url")),
        context_before: string(value.get("context_before")),
        context_after: string(value.get("context_after")),
    }
}

fn relationship(value: &Value, default_source_id: &str) -> SemanticRelationship {
    SemanticRelationship {
        source_id: {
            let value = string(value.get("source_id").or_else(|| value.get("source")));
            if value.is_empty() {
                default_source_id.to_string()
            } else {
                value
            }
        },
        target_id: string(value.get("target_id").or_else(|| value.get("target"))),
        relation_type: {
            let value = string(
                value
                    .get("relation_type")
                    .or_else(|| value.get("type"))
                    .or_else(|| value.get("relation")),
            );
            if value.is_empty() {
                "related".to_string()
            } else {
                value
            }
        },
        properties: object(value.get("properties").or_else(|| value.get("metadata"))),
    }
}

fn projection_entity(value: &Value) -> ProjectionEntity {
    let metadata = object(value.get("metadata"));
    let raw_relationships = value
        .get("relationships")
        .or_else(|| metadata.get("relationships"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let relationships = raw_relationships
        .iter()
        .map(|item| {
            relationship(
                item,
                &string(value.get("id").or_else(|| value.get("kg_id"))),
            )
        })
        .collect();
    let mentions = value
        .get("mentions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    if let Some(span) = item
                        .get("spans")
                        .and_then(Value::as_array)
                        .and_then(|spans| spans.first())
                    {
                        mention_span(span)
                    } else {
                        mention_span(item)
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let title = {
        let value = string(
            value
                .get("label")
                .or_else(|| value.get("title"))
                .or_else(|| value.get("id")),
        );
        if value.is_empty() {
            "Untitled".to_string()
        } else {
            value
        }
    };
    let kg_id = {
        let value = string(value.get("id").or_else(|| value.get("kg_id")));
        if value.is_empty() {
            title.clone()
        } else {
            value
        }
    };
    let body = string(metadata.get("body").or_else(|| value.get("body")));
    ProjectionEntity {
        kg_id,
        title,
        entity_type: {
            let value = string(
                value
                    .get("type")
                    .or_else(|| value.get("entity_type"))
                    .or_else(|| metadata.get("entity_type")),
            );
            if value.is_empty() {
                "note".to_string()
            } else {
                value
            }
        },
        summary: string(value.get("summary").or_else(|| metadata.get("summary"))),
        metadata,
        source_ids: strings(value.get("source_ids")),
        target_ids: strings(value.get("target_ids")),
        relation: value
            .get("relation")
            .and_then(Value::as_str)
            .map(str::to_string),
        relationships,
        mentions,
        body,
        version: number(value.get("version")),
        event_seq: number(value.get("event_seq")),
    }
}

pub fn from_export_file(path: &Path) -> Result<ProviderSnapshot> {
    let raw = std::fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&raw)?;
    let entities = value
        .get("entities")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("entities must be a JSON array"))?
        .iter()
        .map(projection_entity)
        .collect();
    Ok(ProviderSnapshot {
        entities,
        version: number(value.get("version")),
        event_seq: number(value.get("event_seq")),
    })
}

pub fn from_entity_projection(raw: &str) -> Result<ProviderSnapshot> {
    let value: Value = serde_json::from_str(raw)?;
    let entities_object = match value.get("entities") {
        Some(Value::Object(map)) => map,
        _ => bail!("entity projection entities must be an object"),
    };
    let mut entities = Vec::new();
    for row in entities_object.values() {
        if row.get("deleted").and_then(Value::as_bool) != Some(false) {
            continue;
        }
        let Some(entity) = row.get("entity") else {
            continue;
        };
        let mut entity = projection_entity(entity);
        if entity.event_seq.is_none() {
            entity.event_seq = number(row.get("seq"));
        }
        if entity.version.is_none() {
            entity.version = entity.event_seq;
        }
        entities.push(entity);
    }
    entities.sort_by(|left, right| left.kg_id.cmp(&right.kg_id));
    let last_seq = number(value.get("last_seq"));
    Ok(ProviderSnapshot {
        entities,
        version: last_seq,
        event_seq: last_seq,
    })
}

pub fn iter_related_ids(snapshot: &ProviderSnapshot, entity_id: &str) -> Vec<String> {
    let Some(entity) = snapshot
        .entities
        .iter()
        .find(|entity| entity.kg_id == entity_id)
    else {
        return Vec::new();
    };
    let mut related = entity.source_ids.clone();
    related.extend(entity.target_ids.clone());
    for relationship in &entity.relationships {
        if !relationship.source_id.is_empty() {
            related.push(relationship.source_id.clone());
        }
        if !relationship.target_id.is_empty() {
            related.push(relationship.target_id.clone());
        }
    }
    related
}

pub fn short_id(value: &str) -> String {
    let mut digest = Sha1::new();
    digest.update(value.as_bytes());
    sha1::Sha1::digest(digest.finalize())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()[..8]
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_graph_export() {
        let value = serde_json::json!({
            "version": 42,
            "event_seq": 98,
            "entities": [{
                "id": "node:a", "label": "A", "type": "concept",
                "source_ids": [], "target_ids": ["node:b"]
            }]
        });
        let snapshot = from_entity_projection(&serde_json::to_string(&value).unwrap()).unwrap_err();
        assert!(snapshot.to_string().contains("entities must be an object"));
    }

    #[test]
    fn entity_projection_adapter_filters_tombstones() {
        let value = serde_json::json!({
            "last_seq": 7,
            "entities": {
                "alpha": {"deleted": true, "seq": 7},
                "beta": {"deleted": false, "seq": 5, "entity": {"id": "beta", "label": "Beta", "type": "concept"}}
            }
        });
        let snapshot = from_entity_projection(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(snapshot.event_seq, Some(7));
        assert_eq!(snapshot.entities.len(), 1);
        assert_eq!(snapshot.entities[0].kg_id, "beta");
        assert_eq!(snapshot.entities[0].entity_type, "concept");
    }

    #[test]
    fn short_id_is_stable() {
        assert_eq!(short_id("node:concept:alpha").len(), 8);
    }
}
