# Rust Port Fine Tracklist

| Task | Implementation Detail | Status | Gate |
| --- | --- | --- | --- |
| Graph export adapter | Mirror `integrations/kogwistar_adapter.py::from_export_file`; read `entities`, `version`, `event_seq` | DONE | Export adapter unit test |
| Entity projection adapter | Mirror `from_entity_projection`; filter tombstones, default event/version checkpoints, sort by stable ID | DONE | Projection adapter unit test |
| Entity coercion | Mirror duck-typed entity coercion; label/title/id fallbacks, entity type fallbacks, metadata body/summary | DONE | Coercion tests |
| Relationship coercion | Mirror `SemanticRelationship` conversion; source/target aliases, relation-type fallback, properties/metadata mapping | DONE | Relationship tests |
| Mention coercion | Mirror `MentionSpan`; nested `spans` extraction, page/char defaults, document URL and context fields | DONE | Mention tests |
| Stable paths and duplicate titles | Mirror `ObsidianVaultSink._allocate_paths`; type folders, safe titles, deterministic disambiguation, short SHA-1 IDs | DONE | Path collision tests |
| Note rendering | Mirror `sinks/obsidian.py`; frontmatter, Summary, Body, Links, Semantic Relationships, Heading/Block References, Attachments, Provenance, User Notes | DONE | Golden note fixture pending |
| User-owned block | Preserve `USER-OWNED-START`/`USER-OWNED-END` semantics and editable-section policy | DONE | Round-trip edit test |
| Canvas rendering | Mirror `_render_canvas`; related IDs, deterministic node geometry and edge IDs | DONE | Canvas fixture pending |
| Internal link extraction | Mirror `core/links.py`; strip frontmatter, wikilinks, markdown links, URL decoding, fragments and aliases | DONE | Link extraction tests |
| Ledger persistence | Mirror `ProjectionRecord`; path map, by-ID map, projection kind, version, event sequence and sync mode | DONE | Ledger fixture |
| Materialized state | Mirror `_write_materialized_state`; entity dump plus snapshot version and event sequence | DONE | State fixture |
| Event consumer | Mirror `cdc/event_consumer.py`; line filtering, sequence bounds, upsert/delete, inbox append, replay protection | DONE | Event stream fixture |
| Safe round-trip parser | Mirror `roundtrip/safe_notes.py`; extract `kg_id` and user-owned notes block | DONE | Parser unit test |
| In-memory demo | Mirror `demo/in_memory_obsidian_demo.py` end-to-end projection | DONE | Demo smoke test |
| Multimodal grounding | Preserve multimodal mentions without requiring text spans | DONE | Multimodal grounding test |
| Python parity | Golden graph exports and event streams against Python-generated notes, canvases, ledger and state | DONE | Cross-language fixture suite |
| CI | Add fmt, clippy, test and cross-language parity workflow | DONE | Workflow green |
