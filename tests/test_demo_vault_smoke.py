from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest


def test_build_from_export_smoke_exports_a_real_vault():
    if sys.version_info < (3, 11):
        pytest.skip("The sink targets Python 3.11+, so the end-to-end smoke test only runs there.")

    repo_root = Path(__file__).resolve().parents[1]
    export_path = repo_root / "examples" / "complex_graph_export.json"
    vault_path = repo_root / "tmp_smoke_demo_vault"
    shutil.rmtree(vault_path, ignore_errors=True)

    try:
        result = subprocess.run(
            [
                sys.executable,
                "-m",
                "kogwistar_obsidian_sink.cli",
                "build-from-export",
                "--input",
                str(export_path),
                "--vault",
                str(vault_path),
            ],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
        )

        stats = json.loads(result.stdout)
        assert stats["notes"] == 10
        assert stats["canvases"] == 10
        assert stats["dangling_links"] >= 0

        assert (vault_path / "System" / "ledger.json").exists()
        assert (vault_path / "System" / "index.md").exists()
        assert (vault_path / "Views" / "Hypergraph RAG.canvas").exists()
        assert (vault_path / "Views" / "Progress Log.canvas").exists()

        ledger = json.loads((vault_path / "System" / "ledger.json").read_text(encoding="utf-8"))
        assert Path(ledger["by_id"]["node:document:progress-log"]).as_posix() == "Documents/Progress Log.md"
        assert Path(ledger["by_id"]["node:run:progress-log"]).as_posix() == "Runs/Progress Log.md"

        hypergraph = (vault_path / "Concepts" / "Hypergraph RAG.md").read_text(encoding="utf-8")
        assert "[[LLM Knowledge Bases]]" in hypergraph
        assert "[[Obsidian Sink]]" in hypergraph
        assert "## Semantic Relationships" in hypergraph
        assert "depends_on" in hypergraph
        assert "contradicts" in hypergraph

        document = (vault_path / "Documents" / "LLM Knowledge Bases.md").read_text(encoding="utf-8")
        assert "## Attachments" in document
        assert "[[Assets/llm-knowledge-bases.pdf]]" in document
        assert "[[Assets/diagrams/knowledge-flow.png]]" in document

        project = (vault_path / "Projects" / "Obsidian Sink.md").read_text(encoding="utf-8")
        assert "## Heading References" in project
        assert "## Block References" in project
        assert "[[Obsidian Compatibility Contract#Compatibility Contract]]" in project
        assert "[[Projection Build#^rebuild-loop]]" in project

        index = (vault_path / "System" / "index.md").read_text(encoding="utf-8")
        assert "Documents/Progress Log" in index
        assert "Runs/Progress Log" in index
    finally:
        shutil.rmtree(vault_path, ignore_errors=True)


def test_build_in_memory_demo_exports_and_streams_a_real_vault():
    if sys.version_info < (3, 11):
        pytest.skip("The sink targets Python 3.11+, so the end-to-end smoke test only runs there.")

    repo_root = Path(__file__).resolve().parents[1]
    vault_path = repo_root / "tmp_in_memory_demo_vault"
    shutil.rmtree(vault_path, ignore_errors=True)
    try:
        result = subprocess.run(
            [
                sys.executable,
                "-m",
                "kogwistar_obsidian_sink.cli",
                "build-in-memory-demo",
                "--vault",
                str(vault_path),
            ],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
        )

        payload = json.loads(result.stdout)
        full_dump = payload["full_dump"]
        stream_update = payload["stream_update"]
        assert full_dump["notes"] == 5
        assert full_dump["canvases"] == 5
        assert stream_update["notes"] == 6
        assert stream_update["updated_notes"] >= 1
        assert stream_update["updated_canvases"] >= 1

        project = (vault_path / "Projects" / "Obsidian Sink Demo.md").read_text(encoding="utf-8")
        assert "## Semantic Relationships" in project
        assert "depends_on" in project
        assert "contradicts" in project
        assert "[[In-Memory KG]]" in project

        quickstart = (vault_path / "Documents" / "Quickstart.md").read_text(encoding="utf-8")
        assert "## Heading References" in quickstart
        assert "## Block References" in quickstart
        assert "[[Obsidian Sink Demo#Summary|Summary]]" in quickstart
        assert "[[Obsidian Sink Demo#^rebuild-loop|rebuild loop]]" in quickstart

        inbox = (vault_path / "Workflows" / "Streaming Inbox.md").read_text(encoding="utf-8")
        assert "Incremental Updates" in inbox
        assert "emits" in inbox
    finally:
        shutil.rmtree(vault_path, ignore_errors=True)
