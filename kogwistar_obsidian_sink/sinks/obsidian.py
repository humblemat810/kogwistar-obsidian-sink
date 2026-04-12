from __future__ import annotations

import hashlib
import json
import logging
from collections import Counter, defaultdict
from dataclasses import asdict
from pathlib import Path
from typing import Iterable

from ..core.models import ProjectionEntity, ProjectionRecord, SemanticRelationship
from ..core.provider import ProjectionProvider
from ..core.utils import dump_json, load_json, slugify, write_atomic

LOGGER = logging.getLogger(__name__)


ENTITY_DIRS = {
    "concept": "Concepts",
    "document": "Documents",
    "project": "Projects",
    "person": "People",
    "workflow": "Workflows",
    "run": "Runs",
    "conversation": "Conversations",
    "governance": "Governance",
}


class ObsidianVaultSink:
    def __init__(self, vault_root: str | Path):
        self.vault_root = Path(vault_root)
        self.system_dir = self.vault_root / "System"
        self.views_dir = self.vault_root / "Views"
        self.ledger_path = self.system_dir / "ledger.json"
        self.state_path = self.system_dir / "materialized_state.json"
        self.inbox_path = self.system_dir / "inbox.jsonl"

    def build(self, provider: ProjectionProvider) -> dict[str, int]:
        snapshot = provider.snapshot()
        ledger = load_json(self.ledger_path, {"records": {}, "by_id": {}})
        entities = list(snapshot.entities)
        path_by_id = self._allocate_paths(entities, ledger)
        title_by_id = {entity.kg_id: entity.title for entity in entities}
        title_counts = Counter(entity.title for entity in entities)
        written = 0
        canvases = 0
        dangling_links = 0

        for entity in entities:
            path = path_by_id[entity.kg_id]
            render_result = self._render_note(
                entity,
                snapshot.event_seq,
                snapshot.version,
                path_by_id=path_by_id,
                title_by_id=title_by_id,
                title_counts=title_counts,
            )
            dangling_links += render_result["dangling_links"]
            write_atomic(path, render_result["text"])
            record = ProjectionRecord(
                kg_id=entity.kg_id,
                file_path=path.relative_to(self.vault_root).as_posix(),
                canvas_path=(self.views_dir / f"{self._safe_title(entity.title)}.canvas").relative_to(self.vault_root).as_posix(),
                title=entity.title,
                projection_kind="note",
                last_projected_version=snapshot.version,
                last_applied_event_seq=snapshot.event_seq,
                sync_mode="round_trip_safe_notes",
            )
            ledger["records"][record.file_path] = asdict(record)
            ledger["by_id"][entity.kg_id] = record.file_path
            written += 1

        for entity in entities:
            canvas_path = self.views_dir / f"{self._safe_title(entity.title)}.canvas"
            write_atomic(canvas_path, self._render_canvas(entity, provider))
            canvases += 1

        index_path = self.system_dir / "index.md"
        write_atomic(
            index_path,
            self._render_index(
                entities,
                snapshot.version,
                snapshot.event_seq,
                path_by_id=path_by_id,
                title_counts=title_counts,
            ),
        )
        dump_json(self.ledger_path, ledger)
        self._write_materialized_state(snapshot.version, snapshot.event_seq, entities)
        return {"notes": written, "canvases": canvases, "dangling_links": dangling_links}

    def sync(
        self,
        provider: ProjectionProvider,
        *,
        changed_ids: set[str] | None = None,
        deleted_ids: set[str] | None = None,
        affected_titles: set[str] | None = None,
    ) -> dict[str, int]:
        snapshot = provider.snapshot()
        ledger = load_json(self.ledger_path, {"records": {}, "by_id": {}})
        entities = list(snapshot.entities)
        path_by_id = self._allocate_paths(entities, ledger)
        title_by_id = {entity.kg_id: entity.title for entity in entities}
        title_counts = Counter(entity.title for entity in entities)
        changed_ids = set(changed_ids or set())
        deleted_ids = set(deleted_ids or set())
        affected_titles = set(affected_titles or set())
        impacted_ids = self._compute_impacted_ids(
            entities,
            changed_ids=changed_ids,
            deleted_ids=deleted_ids,
            affected_titles=affected_titles,
        )

        deleted_notes = 0
        deleted_canvases = 0
        for entity_id in deleted_ids:
            record = ledger["records"].pop(ledger["by_id"].pop(entity_id, ""), None)
            if not record:
                continue
            note_path = self.vault_root / Path(record["file_path"])
            if note_path.exists():
                note_path.unlink()
            canvas_rel = record.get("canvas_path")
            if canvas_rel:
                canvas_path = self.vault_root / Path(canvas_rel)
                if canvas_path.exists():
                    canvas_path.unlink()
                    deleted_canvases += 1
            deleted_notes += 1

        written = 0
        canvases = 0
        dangling_links = 0
        for entity in entities:
            if entity.kg_id not in impacted_ids:
                continue
            path = path_by_id[entity.kg_id]
            render_result = self._render_note(
                entity,
                snapshot.event_seq,
                snapshot.version,
                path_by_id=path_by_id,
                title_by_id=title_by_id,
                title_counts=title_counts,
            )
            dangling_links += render_result["dangling_links"]
            if self._write_if_changed(path, render_result["text"]):
                written += 1
            canvas_path = self.views_dir / f"{self._safe_title(entity.title)}.canvas"
            if self._write_if_changed(canvas_path, self._render_canvas(entity, provider)):
                canvases += 1
            record = ProjectionRecord(
                kg_id=entity.kg_id,
                file_path=path.relative_to(self.vault_root).as_posix(),
                projection_kind="note",
                canvas_path=canvas_path.relative_to(self.vault_root).as_posix(),
                title=entity.title,
                last_projected_version=snapshot.version,
                last_applied_event_seq=snapshot.event_seq,
                sync_mode="streaming_incremental",
            )
            ledger["records"][record.file_path] = asdict(record)
            ledger["by_id"][entity.kg_id] = record.file_path

        index_path = self.system_dir / "index.md"
        self._write_if_changed(
            index_path,
            self._render_index(
                entities,
                snapshot.version,
                snapshot.event_seq,
                path_by_id=path_by_id,
                title_counts=title_counts,
            ),
        )
        dump_json(self.ledger_path, ledger)
        self._write_materialized_state(snapshot.version, snapshot.event_seq, entities)
        return {
            "notes": len(entities),
            "canvases": len(entities),
            "updated_notes": written,
            "updated_canvases": canvases,
            "deleted_notes": deleted_notes,
            "deleted_canvases": deleted_canvases,
            "dangling_links": dangling_links,
        }

    def _allocate_paths(self, entities: list[ProjectionEntity], ledger: dict) -> dict[str, Path]:
        existing_by_id = dict(ledger.get("by_id", {}))
        path_by_id: dict[str, Path] = {}
        reserved: set[str] = set()
        unassigned_groups: dict[tuple[str, str], list[ProjectionEntity]] = defaultdict(list)

        for entity in entities:
            existing = existing_by_id.get(entity.kg_id)
            if existing:
                rel_path = Path(existing)
                path_by_id[entity.kg_id] = self.vault_root / rel_path
                reserved.add(rel_path.as_posix())
                continue
            folder = ENTITY_DIRS.get(entity.entity_type.lower(), "Notes")
            stem = self._safe_title(entity.title)
            unassigned_groups[(folder, stem)].append(entity)

        for (folder, stem) in sorted(unassigned_groups):
            group = sorted(unassigned_groups[(folder, stem)], key=lambda item: item.kg_id)
            base_rel = Path(folder) / f"{stem}.md"
            if base_rel.as_posix() not in reserved:
                first = group.pop(0)
                path_by_id[first.kg_id] = self.vault_root / base_rel
                reserved.add(base_rel.as_posix())
            for entity in group:
                rel_path = self._disambiguated_path(folder, stem, entity.kg_id, reserved)
                path_by_id[entity.kg_id] = self.vault_root / rel_path
                reserved.add(rel_path.as_posix())

        return path_by_id

    def _disambiguated_path(self, folder: str, stem: str, kg_id: str, reserved: set[str]) -> Path:
        suffix = self._short_id(kg_id)
        candidate = Path(folder) / f"{stem}__{suffix}.md"
        while candidate.as_posix() in reserved:
            suffix = self._short_id(f"{kg_id}:{suffix}")
            candidate = Path(folder) / f"{stem}__{suffix}.md"
        return candidate

    @staticmethod
    def _safe_title(title: str) -> str:
        safe = " ".join(title.strip().split())
        safe = safe.translate(
            str.maketrans({
                "/": "-",
                "\\": "-",
                ":": "-",
                "*": "-",
                "?": "-",
                "\"": "'",
                "<": "(",
                ">": ")",
                "|": "-",
            })
        )
        safe = safe.rstrip(" .")
        return safe or "Untitled"

    @staticmethod
    def _short_id(value: str) -> str:
        return hashlib.sha1(value.encode("utf-8")).hexdigest()[:8]

    def _write_materialized_state(self, version: int | None, event_seq: int | None, entities: list[ProjectionEntity]) -> None:
        payload = {
            "version": version,
            "event_seq": event_seq,
            "entities": [asdict(entity) for entity in entities],
        }
        dump_json(self.state_path, payload)

    def _compute_impacted_ids(
        self,
        entities: list[ProjectionEntity],
        *,
        changed_ids: set[str],
        deleted_ids: set[str],
        affected_titles: set[str],
    ) -> set[str]:
        title_to_ids: dict[str, set[str]] = defaultdict(set)
        for entity in entities:
            title_to_ids[entity.title].add(entity.kg_id)

        impacted: set[str] = set(changed_ids) | set(deleted_ids)
        for title in affected_titles:
            impacted.update(title_to_ids.get(title, set()))

        seed_ids = set(impacted)
        for entity in entities:
            refs = self._entity_reference_ids(entity)
            if refs.intersection(seed_ids):
                impacted.add(entity.kg_id)
        return impacted

    @staticmethod
    def _entity_reference_ids(entity: ProjectionEntity) -> set[str]:
        refs: set[str] = set(entity.source_ids)
        refs.update(entity.target_ids)
        for ref in entity.metadata.get("heading_refs", []) or []:
            if isinstance(ref, dict):
                target_id = str(ref.get("target_id") or ref.get("id") or ref.get("kg_id") or "")
                if target_id:
                    refs.add(target_id)
        for ref in entity.metadata.get("block_refs", []) or []:
            if isinstance(ref, dict):
                target_id = str(ref.get("target_id") or ref.get("id") or ref.get("kg_id") or "")
                if target_id:
                    refs.add(target_id)
        for relationship in entity.relationships:
            if relationship.source_id:
                refs.add(relationship.source_id)
            if relationship.target_id:
                refs.add(relationship.target_id)
        return refs

    @staticmethod
    def _write_if_changed(path: Path, text: str) -> bool:
        if path.exists() and path.read_text(encoding="utf-8") == text:
            return False
        write_atomic(path, text)
        return True

    def _render_note(
        self,
        entity: ProjectionEntity,
        event_seq: int | None,
        version: int | None,
        *,
        path_by_id: dict[str, Path],
        title_by_id: dict[str, str],
        title_counts: Counter[str],
    ) -> dict[str, object]:
        tags = entity.metadata.get("tags", []) or []
        aliases = entity.metadata.get("aliases", []) or []
        attachments = entity.metadata.get("attachments", []) or []
        heading_refs = entity.metadata.get("heading_refs", []) or []
        block_refs = entity.metadata.get("block_refs", []) or []
        current_path = path_by_id[entity.kg_id]
        related_lines, dangling_links = self._render_internal_links(
            entity,
            current_path=current_path,
            path_by_id=path_by_id,
            title_by_id=title_by_id,
            title_counts=title_counts,
        )
        semantic_relationship_lines, semantic_dangling_links = self._render_semantic_relationships(
            entity,
            current_path=current_path,
            path_by_id=path_by_id,
            title_by_id=title_by_id,
            title_counts=title_counts,
        )
        dangling_links += semantic_dangling_links
        lines = [
            "---",
            f"kg_id: {json.dumps(entity.kg_id, ensure_ascii=False)}",
            f"kg_type: {json.dumps(entity.entity_type, ensure_ascii=False)}",
            f"kg_projection: {json.dumps('obsidian_markdown_v1', ensure_ascii=False)}",
            f"kg_version: {version if version is not None else 'null'}",
            f"kg_last_event_seq: {event_seq if event_seq is not None else 'null'}",
            f"kg_source_of_truth: {json.dumps('kogwistar', ensure_ascii=False)}",
            'kg_edit_policy:',
            '  summary: mixed',
            '  notes: user_owned',
            '  structure: read_only',
            f"title: {json.dumps(entity.title, ensure_ascii=False)}",
            f"tags: {json.dumps(tags, ensure_ascii=False)}",
            f"aliases: {json.dumps(aliases, ensure_ascii=False)}",
            "---",
            "",
            f"# {entity.title}",
            "",
            "## Summary",
            entity.summary or "",
            "",
            "## Body",
            entity.body or "",
            "",
            "## Links",
        ]
        related: list[str] = []
        related.extend(related_lines)
        if entity.relation:
            related.append(f"- relation: `{entity.relation}`")
        lines.extend(related or ["- none"])

        if semantic_relationship_lines:
            lines.extend(["", "## Semantic Relationships"])
            lines.extend(semantic_relationship_lines)

        if heading_refs:
            lines.extend(["", "## Heading References"])
            for ref in heading_refs:
                lines.append(
                    self._render_ref_link(
                        ref,
                        path_by_id=path_by_id,
                        title_by_id=title_by_id,
                        kind="heading",
                        title_counts=title_counts,
                    )
                )

        if block_refs:
            lines.extend(["", "## Block References"])
            for ref in block_refs:
                lines.append(
                    self._render_ref_link(
                        ref,
                        path_by_id=path_by_id,
                        title_by_id=title_by_id,
                        kind="block",
                        title_counts=title_counts,
                    )
                )

        if attachments:
            lines.extend(["", "## Attachments"])
            for ref in attachments:
                lines.append(self._render_attachment_link(ref))

        lines.extend(["", "## Provenance"])
        if entity.mentions:
            for mention in entity.mentions:
                excerpt = mention.excerpt.replace("\n", " ").strip()
                lines.append(
                    f"- doc `{mention.doc_id}` page {mention.page_number} chars {mention.start_char}:{mention.end_char} — {excerpt}"
                )
        else:
            lines.append("- none")
        lines.extend(
            [
                "",
                "## User Notes",
                "<!-- USER-OWNED-START -->",
                entity.metadata.get("user_notes", ""),
                "<!-- USER-OWNED-END -->",
                "",
                "```kogwistar-meta",
                "rendered_from: obsidian_sink_v1",
                f"entity_type: {entity.entity_type}",
                f"relation_handles: {entity.source_ids + entity.target_ids}",
                "editable_sections:",
                "  - User Notes",
                "```",
                "",
            ]
        )
        return {"text": "\n".join(lines), "dangling_links": dangling_links}

    def _render_internal_links(
        self,
        entity: ProjectionEntity,
        *,
        current_path: Path,
        path_by_id: dict[str, Path],
        title_by_id: dict[str, str],
        title_counts: Counter[str],
    ) -> tuple[list[str], int]:
        related: list[str] = []
        dangling_links = 0

        for item in entity.source_ids:
            related.append(
                self._render_entity_link(
                    item,
                    "source",
                    current_path=current_path,
                    path_by_id=path_by_id,
                    title_by_id=title_by_id,
                    title_counts=title_counts,
                )
            )
            if item not in path_by_id:
                dangling_links += 1
        for item in entity.target_ids:
            related.append(
                self._render_entity_link(
                    item,
                    "target",
                    current_path=current_path,
                    path_by_id=path_by_id,
                    title_by_id=title_by_id,
                    title_counts=title_counts,
                )
            )
            if item not in path_by_id:
                dangling_links += 1
        return related, dangling_links

    def _render_semantic_relationships(
        self,
        entity: ProjectionEntity,
        *,
        current_path: Path,
        path_by_id: dict[str, Path],
        title_by_id: dict[str, str],
        title_counts: Counter[str],
    ) -> tuple[list[str], int]:
        relationships = list(entity.relationships)
        if not relationships:
            return [], 0

        lines: list[str] = []
        dangling_links = 0
        for relationship in relationships:
            line, line_dangling = self._render_semantic_relationship(
                relationship,
                current_path=current_path,
                path_by_id=path_by_id,
                title_by_id=title_by_id,
                title_counts=title_counts,
            )
            lines.append(line)
            dangling_links += line_dangling
        return lines, dangling_links

    def _render_semantic_relationship(
        self,
        relationship: SemanticRelationship,
        *,
        current_path: Path,
        path_by_id: dict[str, Path],
        title_by_id: dict[str, str],
        title_counts: Counter[str],
    ) -> tuple[str, int]:
        parts: list[str] = [f"- {relationship.relation_type}:"]
        dangling_links = 0
        source_link = self._render_relationship_endpoint(
            relationship.source_id,
            current_path=current_path,
            path_by_id=path_by_id,
            title_by_id=title_by_id,
            title_counts=title_counts,
            endpoint_role="source",
        )
        target_link = self._render_relationship_endpoint(
            relationship.target_id,
            current_path=current_path,
            path_by_id=path_by_id,
            title_by_id=title_by_id,
            title_counts=title_counts,
            endpoint_role="target",
        )
        if source_link and target_link:
            parts.append(f" {source_link} -> {target_link}")
        elif source_link:
            parts.append(f" {source_link}")
        elif target_link:
            parts.append(f" {target_link}")
        if relationship.properties:
            parts.append(f" | properties: {json.dumps(relationship.properties, ensure_ascii=False)}")
        if relationship.source_id and relationship.source_id not in path_by_id:
            dangling_links += 1
        if relationship.target_id and relationship.target_id not in path_by_id:
            dangling_links += 1
        return "".join(parts), dangling_links

    def _render_relationship_endpoint(
        self,
        entity_id: str,
        *,
        current_path: Path,
        path_by_id: dict[str, Path],
        title_by_id: dict[str, str],
        title_counts: Counter[str],
        endpoint_role: str,
    ) -> str:
        if not entity_id:
            return ""
        target_path = path_by_id.get(entity_id)
        if target_path is None:
            LOGGER.warning(
                "Dangling semantic relationship %s endpoint from %s to missing target %s",
                endpoint_role,
                current_path,
                entity_id,
            )
            link_target = self._fallback_link_target(entity_id)
            display = entity_id
        else:
            link_target = self._wiki_target(target_path, title_by_id.get(entity_id, target_path.stem), title_counts)
            display = title_by_id.get(entity_id, target_path.stem)
        return self._format_wikilink(link_target, display)

    def _render_entity_link(
        self,
        target_id: str,
        role: str,
        *,
        current_path: Path,
        path_by_id: dict[str, Path],
        title_by_id: dict[str, str],
        title_counts: Counter[str],
    ) -> str:
        target_path = path_by_id.get(target_id)
        if target_path is None:
            LOGGER.warning("Dangling Obsidian link from %s to missing target %s", current_path, target_id)
            link_target = self._fallback_link_target(target_id)
        else:
            link_target = self._wiki_target(target_path, title_by_id.get(target_id, target_path.stem), title_counts)
        display = title_by_id.get(target_id, target_path.stem if target_path is not None else target_id)
        return f"- {role} -> {self._format_wikilink(link_target, display)}"

    def _render_ref_link(
        self,
        ref: object,
        *,
        path_by_id: dict[str, Path],
        title_by_id: dict[str, str],
        kind: str,
        title_counts: Counter[str],
    ) -> str:
        target_id = ""
        label = ""
        fragment = ""
        if isinstance(ref, dict):
            target_id = str(ref.get("target_id") or ref.get("id") or ref.get("kg_id") or "")
            label = str(ref.get("label") or ref.get("text") or title_by_id.get(target_id, target_id))
            fragment = str(ref.get("heading") or ref.get("block_id") or ref.get("fragment") or "")
        else:
            target_id = str(ref)
            label = title_by_id.get(target_id, target_id)
        target_path = path_by_id.get(target_id)
        if target_path is None:
            LOGGER.warning("Dangling %s reference to missing target %s", kind, target_id)
            link_target = self._fallback_link_target(target_id)
        else:
            link_target = self._wiki_target(target_path, title_by_id.get(target_id, target_path.stem), title_counts)
        if fragment:
            if kind == "block" and not fragment.startswith("^"):
                fragment = f"^{fragment}"
            link_target = f"{link_target}#{fragment}"
        display = None if label == title_by_id.get(target_id, target_id) else label
        return f"- {kind} -> {self._format_wikilink(link_target, display)}"

    @staticmethod
    def _render_attachment_link(ref: object) -> str:
        if isinstance(ref, dict):
            path = str(ref.get("path") or ref.get("file") or ref.get("target") or ref.get("id") or "")
            label = str(ref.get("label") or Path(path).name or path)
        else:
            path = str(ref)
            label = Path(path).name or path
        display = None if label == Path(path).name else label
        return f"- attachment -> {ObsidianVaultSink._format_wikilink(path, display)}"

    @staticmethod
    def _wiki_target(path: Path, title: str, title_counts: Counter[str]) -> str:
        if title_counts.get(title, 0) <= 1:
            return title
        return path.with_suffix("").as_posix()

    @staticmethod
    def _fallback_link_target(target_id: str) -> str:
        return f"Notes/{ObsidianVaultSink._safe_title(target_id)}"

    @staticmethod
    def _format_wikilink(target: str, display: str | None = None) -> str:
        if display and display != target and display != Path(target).name:
            return f"[[{target}|{display}]]"
        return f"[[{target}]]"

    def _render_canvas(self, entity: ProjectionEntity, provider: ProjectionProvider) -> str:
        related = list(dict.fromkeys(provider.iter_related_ids(entity.kg_id)))
        nodes = [
            {
                "id": slugify(entity.kg_id),
                "type": "text",
                "text": entity.title,
                "x": 100,
                "y": 100,
                "width": 260,
                "height": 80,
            }
        ]
        edges = []
        for idx, rel in enumerate(related, start=1):
            nid = slugify(rel)
            nodes.append(
                {
                    "id": nid,
                    "type": "text",
                    "text": rel,
                    "x": 420,
                    "y": 100 + idx * 120,
                    "width": 260,
                    "height": 80,
                }
            )
            edges.append(
                {
                    "id": f"edge-{idx}",
                    "fromNode": slugify(entity.kg_id),
                    "toNode": nid,
                    "fromSide": "right",
                    "toSide": "left",
                }
            )
        payload = {"nodes": nodes, "edges": edges}
        return json.dumps(payload, indent=2)

    def _render_index(
        self,
        entities: Iterable[ProjectionEntity],
        version: int | None,
        event_seq: int | None,
        *,
        path_by_id: dict[str, Path],
        title_counts: Counter[str],
    ) -> str:
        lines = [
            "# Kogwistar Obsidian Sink Index",
            "",
            f"- version: `{version}`",
            f"- event_seq: `{event_seq}`",
            "",
            "## Notes",
        ]
        for entity in sorted(entities, key=lambda item: (item.entity_type, item.title.lower())):
            path = path_by_id[entity.kg_id]
            lines.append(
                f"- {self._format_wikilink(self._wiki_target(path, entity.title, title_counts), entity.title)} ({entity.entity_type})"
            )
        return "\n".join(lines) + "\n"
