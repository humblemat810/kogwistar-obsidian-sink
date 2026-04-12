from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import re


USER_NOTES_RE = re.compile(
    r"<!-- USER-OWNED-START -->(.*?)<!-- USER-OWNED-END -->",
    re.DOTALL,
)
KG_ID_RE = re.compile(r'^kg_id:\s+"([^"]+)"', re.MULTILINE)


@dataclass(slots=True)
class SafeNoteEdit:
    kg_id: str
    user_notes: str


class SafeRoundTripParser:
    def parse(self, path: str | Path) -> SafeNoteEdit:
        text = Path(path).read_text(encoding="utf-8")
        kg = KG_ID_RE.search(text)
        if not kg:
            raise ValueError("Missing kg_id in frontmatter")
        match = USER_NOTES_RE.search(text)
        if not match:
            raise ValueError("Missing user-owned block")
        return SafeNoteEdit(kg_id=kg.group(1), user_notes=match.group(1).strip())
