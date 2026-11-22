# ADR 0001: Native Rust Port for Obsidian Sink

## Status

Accepted.

## Context

The Python sink already owns stable paths, round-trip-safe notes, canvases, and ledger state. Rust must reuse the same user-owned note block and projection identity so Obsidian users can edit without losing provenance.

## Decision

- Use an in-repo Cargo workspace under `rust/`.
- Keep Python packaging and entry points as the compatibility layer.
- Implement export adaptation, entity-projection adaptation, stable path allocation, note/canvas rendering, and build state natively.
- Preserve JSON command names and user-facing output categories.

## Consequences

Rust and Python can be compared with golden graph exports. Ledger and event-driven sync remain tracked as follow-on capabilities.
