from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable, Protocol

from .models import ProjectionEntity


@dataclass(slots=True)
class ProviderSnapshot:
    entities: list[ProjectionEntity]
    version: int | None = None
    event_seq: int | None = None
    metadata: dict[str, str] = field(default_factory=dict)


class ProjectionProvider(Protocol):
    def snapshot(self) -> ProviderSnapshot:
        ...

    def iter_related_ids(self, entity_id: str) -> Iterable[str]:
        ...
