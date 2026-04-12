# Architecture Notes

## Why the split layer exists

This repo deliberately separates:

- **projection-core abstraction**
- **Obsidian file-format/materialization shim**
- **Kogwistar entity/provider shim**

That lets you keep this as a new repo today while still making it easy later to:

- vendor it into the Kogwistar ecosystem
- add a Logseq or generic markdown sink later
- keep Kogwistar authoritative while making sink logic reusable

## Stability strategy against latest Kogwistar main

This repo uses the latest public Kogwistar branch as conceptual source of truth, but keeps the integration seam intentionally conservative:

- no hard dependency on unstable internal storage calls
- assumes Kogwistar can supply entity-like projection outputs
- supports dicts, pydantic objects, dataclasses, and plain Python objects
- materializes deterministic vault files from those objects

This is the safest composition strategy when the substrate is evolving quickly.
