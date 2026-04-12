from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass(slots=True)
class MentionSpan:
    doc_id: str = ""
    page_number: int = 1
    start_char: int = 0
    end_char: int = 0
    excerpt: str = ""
    document_page_url: str = ""
    context_before: str = ""
    context_after: str = ""


@dataclass(slots=True)
class SemanticRelationship:
    source_id: str = ""
    target_id: str = ""
    relation_type: str = "related"
    properties: dict[str, Any] = field(default_factory=dict)


@dataclass(slots=True)
class ProjectionEntity:
    kg_id: str
    title: str
    entity_type: str = "note"
    summary: str = ""
    metadata: dict[str, Any] = field(default_factory=dict)
    source_ids: list[str] = field(default_factory=list)
    target_ids: list[str] = field(default_factory=list)
    relation: str | None = None
    relationships: list[SemanticRelationship] = field(default_factory=list)
    mentions: list[MentionSpan] = field(default_factory=list)
    body: str = ""


@dataclass(slots=True)
class ProjectionRecord:
    kg_id: str
    file_path: str
    projection_kind: str
    last_projected_version: int | None = None
    last_applied_event_seq: int | None = None
    sync_mode: str = "one_way"
