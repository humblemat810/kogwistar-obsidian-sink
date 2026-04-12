from __future__ import annotations

import hashlib
import json
import logging
from pathlib import Path
from tempfile import TemporaryDirectory

from kogwistar_obsidian_sink.core.links import extract_internal_link_targets
from kogwistar_obsidian_sink.core.models import ProjectionEntity, SemanticRelationship
from kogwistar_obsidian_sink.core.provider import ProviderSnapshot, ProjectionProvider
from kogwistar_obsidian_sink.integrations.kogwistar_adapter import KogwistarDuckProvider
from kogwistar_obsidian_sink.roundtrip.safe_notes import SafeRoundTripParser
from kogwistar_obsidian_sink.sinks.obsidian import ObsidianVaultSink


class StubProvider(ProjectionProvider):
    def __init__(self, entities: list[ProjectionEntity], *, version: int | None = None, event_seq: int | None = None):
        self._entities = entities
        self._version = version
        self._event_seq = event_seq

    def snapshot(self) -> ProviderSnapshot:
        return ProviderSnapshot(entities=list(self._entities), version=self._version, event_seq=self._event_seq)

    def iter_related_ids(self, entity_id: str):
        entity = next((item for item in self._entities if item.kg_id == entity_id), None)
        if entity is None:
            return []
        related = list(entity.source_ids + entity.target_ids)
        for relationship in entity.relationships:
            if relationship.source_id:
                related.append(relationship.source_id)
            if relationship.target_id:
                related.append(relationship.target_id)
        return related


def test_internal_link_extraction_recognizes_wikilinks_and_markdown_links():
    text = "Body [[B]] and [B](B.md) and [[B|alias text]]"

    assert extract_internal_link_targets(text) == ["B", "B", "B"]


def test_frontmatter_aliases_alone_do_not_create_edges():
    text = """---
aliases: ["B"]
---

No links here.
"""

    assert extract_internal_link_targets(text) == []


def test_build_writes_notes_and_ledger():
    repo_root = Path(__file__).resolve().parents[1]
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        provider = KogwistarDuckProvider.from_export_file(repo_root / "examples" / "sample_graph_export.json")
        sink = ObsidianVaultSink(vault_root)
        stats = sink.build(provider)

        assert stats["notes"] == 3
        assert stats["canvases"] == 3
        assert (vault_root / "Concepts" / "Hypergraph RAG.md").exists()
        assert (vault_root / "System" / "ledger.json").exists()
        assert (vault_root / "System" / "index.md").exists()
        assert (vault_root / "Views" / "Hypergraph RAG.canvas").exists()

        ledger = json.loads((vault_root / "System" / "ledger.json").read_text(encoding="utf-8"))
        assert ledger["by_id"]["node:concept:hypergraph-rag"] == "Concepts/Hypergraph RAG.md"

        canvas = json.loads((vault_root / "Views" / "Hypergraph RAG.canvas").read_text(encoding="utf-8"))
        assert canvas["nodes"][0]["text"] == "Hypergraph RAG"
        assert len(canvas["edges"]) == 2
        assert {node["text"] for node in canvas["nodes"]} == {
            "Hypergraph RAG",
            "node:document:karpathy-llm-kb",
            "node:project:obsidian-sink",
        }

        note = (vault_root / "Concepts" / "Hypergraph RAG.md").read_text(encoding="utf-8")
        assert "[[LLM Knowledge Bases]]" in note
        assert "[[Obsidian Sink]]" in note
        assert "aliases: [\"Hypergraph Retrieval\"]" in note

        index = (vault_root / "System" / "index.md").read_text(encoding="utf-8")
        assert "Hypergraph RAG" in index
        assert "[[Hypergraph RAG]]" in index


def test_build_is_stable_for_existing_id():
    repo_root = Path(__file__).resolve().parents[1]
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        source = repo_root / "examples" / "sample_graph_export.json"
        provider = KogwistarDuckProvider.from_export_file(source)
        sink = ObsidianVaultSink(vault_root)
        sink.build(provider)

        payload = json.loads(source.read_text(encoding="utf-8"))
        payload["entities"][0]["label"] = "Hypergraph RAG Renamed"
        provider2 = KogwistarDuckProvider(payload["entities"], version=payload["version"], event_seq=payload["event_seq"])
        sink.build(provider2)

        assert (vault_root / "Concepts" / "Hypergraph RAG.md").exists()
        assert not (vault_root / "Concepts" / "Hypergraph RAG Renamed.md").exists()


def test_heading_and_block_refs_and_attachments_are_rendered():
    repo_root = Path(__file__).resolve().parents[1]
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        source = ProjectionEntity(
            kg_id="node:note:source",
            title="Source Note",
            entity_type="note",
            metadata={
                "heading_refs": [{"target_id": "node:note:target", "heading": "Section"}],
                "block_refs": [{"target_id": "node:note:target", "block_id": "block-1"}],
                "attachments": ["Assets/report.pdf", {"path": "Assets/diagram.png"}],
            },
        )
        target = ProjectionEntity(kg_id="node:note:target", title="Target Note", entity_type="note")
        sink = ObsidianVaultSink(vault_root)
        sink.build(StubProvider([source, target]))

        note = (vault_root / "Notes" / "Source Note.md").read_text(encoding="utf-8")
        assert "[[Target Note#Section]]" in note
        assert "[[Target Note#^block-1]]" in note
        assert "[[Assets/report.pdf]]" in note
        assert "[[Assets/diagram.png]]" in note


def test_semantic_relationship_multiplicity_is_preserved():
    repo_root = Path(__file__).resolve().parents[1]
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        source = ProjectionEntity(
            kg_id="node:concept:alpha",
            title="Alpha",
            entity_type="concept",
            relationships=[
                SemanticRelationship(
                    source_id="node:concept:alpha",
                    target_id="node:concept:beta",
                    relation_type="depends_on",
                    properties={"confidence": 0.9},
                ),
                SemanticRelationship(
                    source_id="node:concept:alpha",
                    target_id="node:concept:beta",
                    relation_type="contradicts",
                    properties={"confidence": 0.2, "note": "conflicting claim"},
                ),
            ],
        )
        target = ProjectionEntity(kg_id="node:concept:beta", title="Beta", entity_type="concept")
        sink = ObsidianVaultSink(vault_root)
        sink.build(StubProvider([source, target]))

        note = (vault_root / "Concepts" / "Alpha.md").read_text(encoding="utf-8")
        assert "## Semantic Relationships" in note
        assert "depends_on" in note
        assert "contradicts" in note
        assert "confidence" in note
        assert note.count("[[Alpha]]") >= 2
        assert note.count("[[Beta]]") >= 2


def test_duplicate_titles_are_disambiguated_safely():
    repo_root = Path(__file__).resolve().parents[1]
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        first = ProjectionEntity(kg_id="node:concept:dup-a", title="Duplicate Note", entity_type="concept")
        second = ProjectionEntity(kg_id="node:concept:dup-b", title="Duplicate Note", entity_type="concept")
        sink = ObsidianVaultSink(vault_root)
        sink.build(StubProvider([first, second]))

        base = vault_root / "Concepts" / "Duplicate Note.md"
        suffix = vault_root / "Concepts" / f"Duplicate Note__{hashlib.sha1(second.kg_id.encode('utf-8')).hexdigest()[:8]}.md"

        assert base.exists()
        assert suffix.exists()


def test_filename_sanitization_is_consistent():
    repo_root = Path(__file__).resolve().parents[1]
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        entity = ProjectionEntity(
            kg_id="node:concept:safe-file",
            title='Bad / Name: *? " < > | .',
            entity_type="concept",
        )
        sink = ObsidianVaultSink(vault_root)
        sink.build(StubProvider([entity]))

        safe_name = sink._safe_title(entity.title)
        assert safe_name == safe_name.strip()
        assert not any(ch in safe_name for ch in '\\/:*?"<>|')
        assert not safe_name.endswith(".")
        assert not safe_name.endswith(" ")
        assert (vault_root / "Concepts" / f"{safe_name}.md").exists()


def test_dangling_targets_are_allowed_and_logged(caplog):
    repo_root = Path(__file__).resolve().parents[1]
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        entity = ProjectionEntity(
            kg_id="node:concept:with-dangling",
            title="Dangling Source",
            entity_type="concept",
            source_ids=["node:concept:missing"],
        )
        sink = ObsidianVaultSink(vault_root)
        with caplog.at_level(logging.WARNING):
            stats = sink.build(StubProvider([entity]))

        note = (vault_root / "Concepts" / "Dangling Source.md").read_text(encoding="utf-8")
        assert "Dangling Obsidian link" in caplog.text
        assert "[[Notes/node-concept-missing|node:concept:missing]]" in note
        assert stats["dangling_links"] == 1


def test_safe_roundtrip_parser_reads_user_notes():
    repo_root = Path(__file__).resolve().parents[1]
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        provider = KogwistarDuckProvider.from_export_file(repo_root / "examples" / "sample_graph_export.json")
        sink = ObsidianVaultSink(vault_root)
        sink.build(provider)
        note = vault_root / "Concepts" / "Hypergraph RAG.md"
        text = note.read_text(encoding="utf-8").replace(
            "<!-- USER-OWNED-START -->\n",
            "<!-- USER-OWNED-START -->\nMy manual note.\n",
            1,
        )
        note.write_text(text, encoding="utf-8")

        parsed = SafeRoundTripParser().parse(note)
        assert parsed.kg_id == "node:concept:hypergraph-rag"
        assert parsed.user_notes == "My manual note."
