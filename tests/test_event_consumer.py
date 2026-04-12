from pathlib import Path
from tempfile import TemporaryDirectory

import json

from kogwistar_obsidian_sink.cdc.event_consumer import JsonlEventConsumer


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def test_event_consumer_builds_vault():
    repo_root = _repo_root()
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        stats = JsonlEventConsumer(vault_root).consume(repo_root / "examples" / "sample_events.jsonl")
        assert stats["notes"] == 2
        assert (vault_root / "Concepts" / "Event Sourcing.md").exists()


def test_event_consumer_can_start_from_sequence():
    repo_root = _repo_root()
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        stats = JsonlEventConsumer(vault_root).consume(
            repo_root / "examples" / "sample_events.jsonl",
            from_seq=1002,
        )
        assert stats["notes"] == 1
        assert not (vault_root / "Concepts" / "Event Sourcing.md").exists()
        assert (vault_root / "Projects" / "Obsidian Sink.md").exists()


def test_event_consumer_streaming_updates_only_impacted_files(monkeypatch):
    repo_root = _repo_root()
    with TemporaryDirectory(dir=repo_root) as tmp_dir:
        vault_root = Path(tmp_dir)
        initial_events = vault_root / "initial.jsonl"
        followup_events = vault_root / "followup.jsonl"

        initial_events.write_text(
            "\n".join(
                [
                    json.dumps(
                        {
                            "type": "entity.upsert",
                            "event_seq": 1,
                            "version": 1,
                            "entity": {
                                "id": "node:concept:alpha",
                                "label": "Alpha",
                                "type": "concept",
                                "summary": "Alpha points at Beta.",
                                "target_ids": ["node:concept:beta"],
                            },
                        }
                    ),
                    json.dumps(
                        {
                            "type": "entity.upsert",
                            "event_seq": 2,
                            "version": 1,
                            "entity": {
                                "id": "node:concept:beta",
                                "label": "Beta",
                                "type": "concept",
                                "summary": "Beta is the target.",
                            },
                        }
                    ),
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        followup_events.write_text(
            json.dumps(
                {
                    "type": "entity.upsert",
                    "event_seq": 3,
                    "version": 2,
                    "entity": {
                        "id": "node:concept:beta",
                        "label": "Beta",
                        "type": "concept",
                        "summary": "Beta has changed and should drive a small incremental refresh.",
                    },
                }
            )
            + "\n",
            encoding="utf-8",
        )

        consumer = JsonlEventConsumer(vault_root)
        consumer.consume(initial_events)

        written_paths: list[str] = []
        utility_write_paths: list[str] = []
        from kogwistar_obsidian_sink.sinks import obsidian as obsidian_sink  # local import to keep test scope small
        from kogwistar_obsidian_sink.core import utils as core_utils

        original_write_atomic = obsidian_sink.write_atomic
        original_utility_write_atomic = core_utils.write_atomic

        def spy_write_atomic(path, text):
            written_paths.append(path.as_posix())
            return original_write_atomic(path, text)

        def spy_utility_write_atomic(path, text):
            utility_write_paths.append(path.as_posix())
            return original_utility_write_atomic(path, text)

        monkeypatch.setattr(obsidian_sink, "write_atomic", spy_write_atomic)
        monkeypatch.setattr(core_utils, "write_atomic", spy_utility_write_atomic)

        stats = consumer.consume(followup_events)

        assert stats["notes"] == 2
        assert stats["canvases"] == 2
        assert len(written_paths) < 10
        assert any(path.endswith("Concepts/Beta.md") for path in written_paths)
        assert any(path.endswith("Concepts/Alpha.md") for path in written_paths)
        assert any(path.endswith("System/index.md") for path in written_paths)
        assert any(path.endswith("System/ledger.json") for path in utility_write_paths)
        assert any(path.endswith("System/materialized_state.json") for path in utility_write_paths)
