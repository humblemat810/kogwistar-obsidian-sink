from __future__ import annotations

import json
import shutil
from collections import defaultdict
from pathlib import Path
from typing import Any

from kogwistar.engine_core.engine import GraphKnowledgeEngine
from kogwistar.engine_core.in_memory_backend import build_in_memory_backend
from kogwistar.engine_core.models import Edge, Grounding, MentionVerification, Node, Span

from ..integrations.kogwistar_adapter import KogwistarDuckProvider
from ..sinks.obsidian import ObsidianVaultSink


class OneDimEmbedding:
    """Tiny embedder for quickstart/demo use."""

    @staticmethod
    def name() -> str:
        return "kogwistar-obsidian-demo-1d"

    def is_legacy(self) -> bool:
        return False

    @staticmethod
    def supported_spaces() -> list[str]:
        return ["cosine"]

    @staticmethod
    def get_config() -> dict[str, object]:
        return {}

    @classmethod
    def build_from_config(
        cls, config: dict[str, object] | None = None
    ) -> "OneDimEmbedding":
        _ = config
        return cls()

    def __call__(self, texts):
        return [[1.0] for _ in texts]


def _span(doc_id: str, excerpt: str, *, insertion_method: str) -> Span:
    return Span(
        collection_page_url=f"demo/{doc_id}",
        document_page_url=f"demo/{doc_id}",
        doc_id=doc_id,
        insertion_method=insertion_method,
        page_number=1,
        start_char=0,
        end_char=max(1, len(excerpt)),
        excerpt=excerpt[:512],
        context_before="",
        context_after="",
        chunk_id=None,
        source_cluster_id=None,
        verification=MentionVerification(
            method="system", is_verified=True, score=1.0, notes=insertion_method
        ),
    )


def _grounding(doc_id: str, excerpt: str, *, insertion_method: str) -> Grounding:
    return Grounding(spans=[_span(doc_id, excerpt, insertion_method=insertion_method)])


def _build_engine(base_dir: Path) -> GraphKnowledgeEngine:
    return GraphKnowledgeEngine(
        persist_directory=str(base_dir / "knowledge"),
        kg_graph_type="knowledge",
        backend_factory=build_in_memory_backend,
        embedding_function=OneDimEmbedding(),
    )


def _make_node(
    *,
    node_id: str,
    label: str,
    entity_type: str,
    summary: str,
    level_from_root: int,
    body: str = "",
    aliases: list[str] | None = None,
    heading_refs: list[dict[str, Any]] | None = None,
    block_refs: list[dict[str, Any]] | None = None,
) -> Node:
    metadata: dict[str, Any] = {
        "entity_type": entity_type,
        "aliases": aliases or [],
        "body": body,
    }
    if heading_refs:
        metadata["heading_refs"] = heading_refs
    if block_refs:
        metadata["block_refs"] = block_refs
    return Node(
        id=node_id,
        label=label,
        type="entity",
        summary=summary,
        doc_id=f"doc:{node_id}",
        mentions=[
            _grounding(f"doc:{node_id}", summary, insertion_method="in_memory_demo")
        ],
        properties={},
        metadata=metadata,
        domain_id=None,
        canonical_entity_id=None,
        level_from_root=level_from_root,
        embedding=None,
    )


def _make_edge(
    *,
    edge_id: str,
    source_id: str,
    target_id: str,
    relation: str,
    summary: str,
) -> Edge:
    return Edge(
        id=edge_id,
        source_ids=[source_id],
        target_ids=[target_id],
        relation=relation,
        label=relation,
        type="relationship",
        summary=summary,
        doc_id=f"doc:{edge_id}",
        mentions=[_grounding(f"doc:{edge_id}", summary, insertion_method="in_memory_demo")],
        properties={},
        metadata={"entity_type": "relationship"},
        source_edge_ids=[],
        target_edge_ids=[],
        domain_id=None,
        canonical_entity_id=None,
        embedding=None,
    )


def _seed_initial_graph(engine: GraphKnowledgeEngine) -> None:
    project_id = "node:project:obsidian-sink-demo"
    kg_id = "node:concept:in-memory-kg"
    inbox_id = "node:workflow:streaming-inbox"
    stable_id = "node:governance:stable-paths"
    guide_id = "node:document:quickstart"

    nodes = [
        _make_node(
            node_id=project_id,
            label="Obsidian Sink Demo",
            entity_type="project",
            summary="End-to-end demo project for dumping and streaming into Obsidian.",
            level_from_root=0,
            body="This is the main demo project note. ^rebuild-loop",
            aliases=["Demo Project"],
        ),
        _make_node(
            node_id=kg_id,
            label="In-Memory KG",
            entity_type="concept",
            summary="The Kogwistar engine runs in memory for the demo.",
            level_from_root=1,
        ),
        _make_node(
            node_id=inbox_id,
            label="Streaming Inbox",
            entity_type="workflow",
            summary="New changes are appended to an inbox before projection.",
            level_from_root=1,
        ),
        _make_node(
            node_id=stable_id,
            label="Stable Paths",
            entity_type="governance",
            summary="Stable filenames keep Obsidian links healthy across reruns.",
            level_from_root=1,
        ),
        _make_node(
            node_id=guide_id,
            label="Quickstart",
            entity_type="document",
            summary="The shortest path from in-memory KG to an Obsidian vault.",
            level_from_root=1,
            heading_refs=[
                {
                    "target_id": project_id,
                    "label": "Summary",
                    "heading": "Summary",
                }
            ],
            block_refs=[
                {
                    "target_id": project_id,
                    "label": "rebuild loop",
                    "block_id": "rebuild-loop",
                }
            ],
            aliases=["Launch Guide"],
        ),
    ]
    for node in nodes:
        engine.write.add_node(node)

    edges = [
        _make_edge(
            edge_id="edge:demo:project-depends-on-kg",
            source_id=project_id,
            target_id=kg_id,
            relation="depends_on",
            summary="The demo project depends on the in-memory KG.",
        ),
        _make_edge(
            edge_id="edge:demo:project-contradicts-kg",
            source_id=project_id,
            target_id=kg_id,
            relation="contradicts",
            summary="The demo project also tracks a conflicting perspective for the same pair.",
        ),
        _make_edge(
            edge_id="edge:demo:project-uses-inbox",
            source_id=project_id,
            target_id=inbox_id,
            relation="uses",
            summary="The demo project uses the streaming inbox pattern.",
        ),
        _make_edge(
            edge_id="edge:demo:stable-supports-project",
            source_id=stable_id,
            target_id=project_id,
            relation="supports",
            summary="Stable paths support the demo project.",
        ),
        _make_edge(
            edge_id="edge:demo:guide-documents-project",
            source_id=guide_id,
            target_id=project_id,
            relation="documents",
            summary="The quickstart documents the demo project.",
        ),
    ]
    for edge in edges:
        engine.write.add_edge(edge)


def _apply_stream_update(engine: GraphKnowledgeEngine) -> tuple[str, str]:
    inbox_id = "node:workflow:streaming-inbox"
    update_id = "node:workflow:incremental-updates"
    update_node = _make_node(
        node_id=update_id,
        label="Incremental Updates",
        entity_type="workflow",
        summary="A new event stream entry materializes only the impacted files.",
        level_from_root=1,
    )
    engine.write.add_node(update_node)
    engine.write.add_edge(
        _make_edge(
            edge_id="edge:demo:inbox-emits-update",
            source_id=inbox_id,
            target_id=update_id,
            relation="emits",
            summary="The streaming inbox emits incremental updates.",
        )
    )
    return inbox_id, update_id


def _engine_to_provider(
    engine: GraphKnowledgeEngine,
    *,
    version: int,
    event_seq: int,
) -> KogwistarDuckProvider:
    nodes = engine.read.get_nodes(limit=10_000, resolve_mode="active_only")
    edges = engine.read.get_edges(limit=10_000, resolve_mode="active_only")

    node_payloads: dict[str, dict[str, Any]] = {}
    outgoing_targets: dict[str, set[str]] = defaultdict(set)
    relationships_by_source: dict[str, list[dict[str, Any]]] = defaultdict(list)

    for node in nodes:
        payload = node.model_dump(field_mode="backend", exclude={"embedding"})
        metadata = dict(payload.get("metadata") or {})
        payload["id"] = str(payload.get("id") or node.safe_get_id())
        payload["label"] = str(payload.get("label") or payload["id"])
        payload["title"] = payload["label"]
        payload["summary"] = str(payload.get("summary") or "")
        payload["metadata"] = metadata
        payload["type"] = str(metadata.get("entity_type") or payload.get("type") or "note")
        payload["source_ids"] = []
        payload["target_ids"] = []
        payload["relationships"] = []
        payload["body"] = str(metadata.get("body") or "")
        node_payloads[payload["id"]] = payload

    for edge in edges:
        payload = edge.model_dump(field_mode="backend", exclude={"embedding"})
        source_ids = [str(v) for v in payload.get("source_ids", []) or []]
        target_ids = [str(v) for v in payload.get("target_ids", []) or []]
        if not source_ids or not target_ids:
            continue
        source_id = source_ids[0]
        target_id = target_ids[0]
        outgoing_targets[source_id].add(target_id)
        relation_type = str(payload.get("relation") or payload.get("type") or "related")
        relationship_properties = dict(payload.get("metadata") or {})
        relationship_properties["edge_id"] = str(payload.get("id") or edge.safe_get_id())
        relationships_by_source[source_id].append(
            {
                "source_id": source_id,
                "target_id": target_id,
                "relation_type": relation_type,
                "properties": relationship_properties,
            }
        )

    for node_id, payload in node_payloads.items():
        payload["target_ids"] = sorted(outgoing_targets.get(node_id, set()))
        payload["relationships"] = relationships_by_source.get(node_id, [])

    return KogwistarDuckProvider(
        list(node_payloads.values()),
        version=version,
        event_seq=event_seq,
    )


def run_end_to_end_demo(vault_root: str | Path) -> dict[str, Any]:
    vault_root = Path(vault_root)
    shutil.rmtree(vault_root, ignore_errors=True)
    vault_root.mkdir(parents=True, exist_ok=True)

    repo_root = Path(__file__).resolve().parents[2]
    work_dir = repo_root / ".demo_work_in_memory"
    shutil.rmtree(work_dir, ignore_errors=True)
    try:
        work_dir.mkdir(parents=True, exist_ok=True)
        engine = _build_engine(work_dir)
        _seed_initial_graph(engine)

        sink = ObsidianVaultSink(vault_root)
        full_stats = sink.build(_engine_to_provider(engine, version=1, event_seq=1))

        inbox_id, update_id = _apply_stream_update(engine)
        stream_stats = sink.sync(
            _engine_to_provider(engine, version=2, event_seq=2),
            changed_ids={inbox_id, update_id},
            affected_titles={"Streaming Inbox", "Incremental Updates"},
        )
    finally:
        shutil.rmtree(work_dir, ignore_errors=True)

    return {
        "vault": str(vault_root),
        "full_dump": full_stats,
        "stream_update": stream_stats,
    }


def main(argv: list[str] | None = None) -> int:
    _ = argv
    payload = run_end_to_end_demo(Path("./demo_vault"))
    print(json.dumps(payload, indent=2))
    return 0
