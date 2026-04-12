from __future__ import annotations

import argparse
import json
from pathlib import Path

from .cdc.event_consumer import JsonlEventConsumer
from .integrations.kogwistar_adapter import KogwistarDuckProvider
from .roundtrip.safe_notes import SafeRoundTripParser
from .sinks.obsidian import ObsidianVaultSink


def build_demo(args: argparse.Namespace) -> int:
    provider = KogwistarDuckProvider.from_export_file(args.sample)
    sink = ObsidianVaultSink(args.vault)
    stats = sink.build(provider)
    print(json.dumps(stats, indent=2))
    return 0


def build_from_export(args: argparse.Namespace) -> int:
    provider = KogwistarDuckProvider.from_export_file(args.input)
    sink = ObsidianVaultSink(args.vault)
    stats = sink.build(provider)
    print(json.dumps(stats, indent=2))
    return 0


def consume_events(args: argparse.Namespace) -> int:
    consumer = JsonlEventConsumer(args.vault)
    stats = consumer.consume(args.events)
    print(json.dumps(stats, indent=2))
    return 0


def inspect_note(args: argparse.Namespace) -> int:
    parser = SafeRoundTripParser()
    edit = parser.parse(args.path)
    print(json.dumps({"kg_id": edit.kg_id, "user_notes": edit.user_notes}, indent=2))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(prog="kogwistar-obsidian-sink")
    sub = parser.add_subparsers(dest="command", required=True)

    p_demo = sub.add_parser("build-demo")
    p_demo.add_argument("--sample", required=True)
    p_demo.add_argument("--vault", required=True)
    p_demo.set_defaults(func=build_demo)

    p_export = sub.add_parser("build-from-export")
    p_export.add_argument("--input", required=True)
    p_export.add_argument("--vault", required=True)
    p_export.set_defaults(func=build_from_export)

    p_events = sub.add_parser("consume-events")
    p_events.add_argument("--events", required=True)
    p_events.add_argument("--vault", required=True)
    p_events.set_defaults(func=consume_events)

    p_inspect = sub.add_parser("inspect-note")
    p_inspect.add_argument("--path", required=True)
    p_inspect.set_defaults(func=inspect_note)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
