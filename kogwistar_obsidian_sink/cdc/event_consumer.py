from __future__ import annotations

import json
from pathlib import Path
from typing import Iterable

from ..integrations.kogwistar_adapter import KogwistarDuckProvider
from ..sinks.obsidian import ObsidianVaultSink


class JsonlEventConsumer:
    """Minimal scaffold for authoritative-event consumption.

    Event format:
    {"type": "entity.upsert", "entity": {...}, "event_seq": 123, "version": 7}
    """

    def __init__(self, vault_root: str | Path):
        self.sink = ObsidianVaultSink(vault_root)

    def consume(self, events_path: str | Path) -> dict[str, int]:
        entities = []
        last_event_seq = None
        last_version = None
        for line in Path(events_path).read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            event = json.loads(line)
            if event.get("type") == "entity.upsert":
                entities.append(event["entity"])
                last_event_seq = event.get("event_seq", last_event_seq)
                last_version = event.get("version", last_version)
        provider = KogwistarDuckProvider(entities=entities, event_seq=last_event_seq, version=last_version)
        return self.sink.build(provider)
