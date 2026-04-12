from __future__ import annotations

import json
from dataclasses import asdict, is_dataclass
from pathlib import Path
from typing import Any, Iterable

from ..core.models import MentionSpan, ProjectionEntity, SemanticRelationship
from ..core.provider import ProjectionProvider, ProviderSnapshot


class KogwistarDuckProvider(ProjectionProvider):
    """Adapt dicts or Kogwistar-like objects using stable, low-assumption duck typing."""

    def __init__(self, entities: Iterable[Any], *, version: int | None = None, event_seq: int | None = None):
        self._raw_entities = list(entities)
        self._version = version
        self._event_seq = event_seq
        self._entities = [self._coerce_entity(item) for item in self._raw_entities]

    @classmethod
    def from_export_file(cls, path: str | Path) -> "KogwistarDuckProvider":
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
        return cls(
            payload.get("entities", []),
            version=payload.get("version"),
            event_seq=payload.get("event_seq"),
        )

    def snapshot(self) -> ProviderSnapshot:
        return ProviderSnapshot(
            entities=list(self._entities),
            version=self._version,
            event_seq=self._event_seq,
            metadata={"provider": "KogwistarDuckProvider"},
        )

    def iter_related_ids(self, entity_id: str):
        entity = next((item for item in self._entities if item.kg_id == entity_id), None)
        if entity is None:
            return []
        related = []
        related.extend(entity.source_ids)
        related.extend(entity.target_ids)
        for relationship in entity.relationships:
            if relationship.source_id:
                related.append(relationship.source_id)
            if relationship.target_id:
                related.append(relationship.target_id)
        return related

    def _coerce_entity(self, item: Any) -> ProjectionEntity:
        if isinstance(item, ProjectionEntity):
            return item
        if is_dataclass(item):
            item = asdict(item)
        if hasattr(item, "model_dump"):
            item = item.model_dump()
        if not isinstance(item, dict):
            item = self._object_to_dict(item)

        mentions = [self._coerce_mention(m) for m in item.get("mentions", []) or []]
        metadata = dict(item.get("metadata") or {})
        raw_relationships = item.get("relationships", []) or metadata.get("relationships", []) or []
        relationships = [
            self._coerce_relationship(r, default_source_id=str(item.get("id") or item.get("kg_id") or ""))
            for r in raw_relationships
        ]
        title = str(item.get("label") or item.get("title") or item.get("id") or "Untitled")
        entity_type = str(item.get("type") or metadata.get("entity_type") or "note")
        body = str(metadata.get("body") or item.get("body") or "")
        return ProjectionEntity(
            kg_id=str(item.get("id") or item.get("kg_id") or title),
            title=title,
            entity_type=entity_type,
            summary=str(item.get("summary") or metadata.get("summary") or ""),
            metadata=metadata,
            source_ids=[str(v) for v in item.get("source_ids", []) or []],
            target_ids=[str(v) for v in item.get("target_ids", []) or []],
            relation=(None if item.get("relation") in (None, "") else str(item.get("relation"))),
            relationships=relationships,
            mentions=mentions,
            body=body,
        )

    def _coerce_mention(self, item: Any) -> MentionSpan:
        if is_dataclass(item):
            item = asdict(item)
        elif hasattr(item, "model_dump"):
            item = item.model_dump()
        elif not isinstance(item, dict):
            item = self._object_to_dict(item)
        spans = item.get("spans") if isinstance(item, dict) else None
        if spans:
            item = spans[0]
        return MentionSpan(
            doc_id=str(item.get("doc_id") or ""),
            page_number=int(item.get("page_number") or 1),
            start_char=int(item.get("start_char") or 0),
            end_char=int(item.get("end_char") or 0),
            excerpt=str(item.get("excerpt") or ""),
            document_page_url=str(item.get("document_page_url") or ""),
            context_before=str(item.get("context_before") or ""),
            context_after=str(item.get("context_after") or ""),
        )

    def _coerce_relationship(self, item: Any, *, default_source_id: str = "") -> SemanticRelationship:
        if is_dataclass(item):
            item = asdict(item)
        elif hasattr(item, "model_dump"):
            item = item.model_dump()
        elif not isinstance(item, dict):
            item = self._object_to_dict(item)
        properties = dict(item.get("properties") or item.get("metadata") or {})
        relation_type = str(item.get("relation_type") or item.get("type") or item.get("relation") or "related")
        return SemanticRelationship(
            source_id=str(item.get("source_id") or item.get("source") or default_source_id),
            target_id=str(item.get("target_id") or item.get("target") or ""),
            relation_type=relation_type,
            properties=properties,
        )

    @staticmethod
    def _object_to_dict(item: Any) -> dict[str, Any]:
        keys = [
            "id", "kg_id", "label", "title", "type", "summary", "metadata", "mentions",
            "source_ids", "target_ids", "relation", "relationships", "body",
        ]
        payload = {}
        for key in keys:
            if hasattr(item, key):
                payload[key] = getattr(item, key)
        return payload
