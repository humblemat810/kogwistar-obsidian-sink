from pathlib import Path

from kogwistar_obsidian_sink.cdc.event_consumer import JsonlEventConsumer


def test_event_consumer_builds_vault(tmp_path: Path):
    stats = JsonlEventConsumer(tmp_path).consume(Path(__file__).parent.parent / "examples" / "sample_events.jsonl")
    assert stats["notes"] == 2
    assert (tmp_path / "Concepts" / "Event Sourcing.md").exists()
