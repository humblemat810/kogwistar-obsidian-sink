from __future__ import annotations

import json
from pathlib import Path

from ..integrations.kogwistar_adapter import KogwistarDuckProvider
from ..sinks.obsidian import ObsidianVaultSink
from ..core.utils import load_json


class JsonlEventConsumer:
    """Minimal scaffold for authoritative-event consumption.

    Event format:
    {"type": "entity.upsert", "entity": {...}, "event_seq": 123, "version": 7}
    """

    def __init__(self, vault_root: str | Path):
        self.sink = ObsidianVaultSink(vault_root)

    def consume(
        self,
        events_path: str | Path,
        *,
        from_seq: int | None = None,
        to_seq: int | None = None,
    ) -> dict[str, int]:
        state = load_json(
            self.sink.state_path,
            {"version": None, "event_seq": None, "entities": []},
        )
        entities_by_id = {
            str(entity.get("kg_id") or entity.get("id") or entity.get("label") or ""): entity
            for entity in state.get("entities", [])
            if isinstance(entity, dict)
        }
        changed_ids: set[str] = set()
        deleted_ids: set[str] = set()
        affected_titles: set[str] = set()
        last_event_seq = state.get("event_seq")
        last_version = state.get("version")
        for line in Path(events_path).read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            event = json.loads(line)
            event_seq = event.get("event_seq")
            if from_seq is not None and (event_seq is None or int(event_seq) < int(from_seq)):
                continue
            if to_seq is not None and (event_seq is None or int(event_seq) > int(to_seq)):
                continue
            if event_seq is not None and last_event_seq is not None and int(event_seq) <= int(last_event_seq):
                continue
            entity = event.get("entity") or {}
            entity_id = str(entity.get("id") or entity.get("kg_id") or entity.get("label") or "")
            if not entity_id:
                continue
            self._append_inbox_event(event)
            event_type = event.get("type")
            if event_type == "entity.upsert":
                previous = entities_by_id.get(entity_id)
                if previous:
                    previous_title = str(previous.get("title") or previous.get("label") or entity_id)
                    affected_titles.add(previous_title)
                new_title = str(entity.get("label") or entity.get("title") or entity_id)
                affected_titles.add(new_title)
                entities_by_id[entity_id] = self._normalize_state_entity(entity)
                changed_ids.add(entity_id)
            elif event_type in {"entity.delete", "entity.remove", "entity.tombstone"}:
                previous = entities_by_id.pop(entity_id, None)
                if previous:
                    previous_title = str(previous.get("title") or previous.get("label") or entity_id)
                    affected_titles.add(previous_title)
                deleted_ids.add(entity_id)
            else:
                continue
            if event_seq is not None:
                last_event_seq = int(event_seq)
            last_version = event.get("version", last_version)
        provider = KogwistarDuckProvider(entities=list(entities_by_id.values()), event_seq=last_event_seq, version=last_version)
        stats = self.sink.sync(
            provider,
            changed_ids=changed_ids,
            deleted_ids=deleted_ids,
            affected_titles=affected_titles,
        )
        return stats

    def _append_inbox_event(self, event: dict) -> None:
        self.sink.inbox_path.parent.mkdir(parents=True, exist_ok=True)
        with self.sink.inbox_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(event, ensure_ascii=False) + "\n")

    @staticmethod
    def _normalize_state_entity(entity: dict) -> dict:
        payload = dict(entity)
        payload.setdefault("metadata", {})
        payload.setdefault("source_ids", [])
        payload.setdefault("target_ids", [])
        payload.setdefault("relationships", [])
        payload.setdefault("mentions", [])
        payload.setdefault("type", payload.get("type") or payload.get("entity_type") or "note")
        payload.setdefault("label", payload.get("label") or payload.get("title") or payload.get("id") or "Untitled")
        return payload
