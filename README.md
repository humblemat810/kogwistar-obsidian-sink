# Kogwistar Obsidian Sink

A standalone, composable repository that turns Kogwistar-style graph artifacts into an Obsidian vault.

This repo is intentionally split into two layers:

1. a **generic vault projection layer** that can target Obsidian today and other markdown/file-based systems later
2. a **Kogwistar integration shim** that adapts Kogwistar-like entities into that projection layer

That keeps the sink usable as a standalone repo while still fitting Kogwistar's projection-oriented architecture.

## What is included

- deterministic markdown note projection
- stable `kg_id -> file path` mapping ledger
- idempotent rebuilds
- generated index pages
- generated Obsidian `.canvas` neighborhood views
- a CDC/event-consumer scaffold
- safe round-trip parsing for explicitly editable sections
- a working demo using a larger Kogwistar-style export
- tests

## Repo layout

```text
kogwistar_obsidian_sink/
  core/           generic projection abstractions
  integrations/   Kogwistar-specific shims and adapters
  sinks/          Obsidian writer
  cdc/            event consumer scaffold
  roundtrip/      safe editable-zone parsing scaffold
examples/         runnable sample export and config
tests/            regression tests
```

## Design stance

- **Authoritative source of truth remains Kogwistar**
- the Obsidian vault is a **durable but rebuildable projection**
- write-back is **constrained** and limited to explicit editable zones
- generated structure is not inferred back into graph mutations by default

## Quick start

### 1. Create a virtual environment

```bash
python -m venv .venv
source .venv/bin/activate
```

On Windows PowerShell:

```powershell
python -m venv .venv
.\.venv\Scripts\Activate.ps1
```

### 2. Install the repo

```bash
pip install -e .
```

For consumer installs, this resolves `kogwistar` from GitHub through the package metadata in `pyproject.toml`.
That is the normal default path.

If you want to work against the checked-out local `./kogwistar` subtree (for AI visibility) instead, run the opt-in bootstrap script after installing the repo:

```bash
bash scripts/bootstrap-dev.sh
```

The bootstrap script is manual on purpose. It only switches the active environment to the local subtree when you run it.
If `./kogwistar` is missing, the script clones it first and then installs it editable.

Windows users can run the same Bash script from Git Bash or WSL.

### 3. Run the end-to-end in-memory demo

```bash
kogwistar-obsidian-sink build-in-memory-demo --vault ./demo_vault
```

You should get:

- `Concepts/*.md`
- `Documents/*.md`
- `Projects/*.md`
- `Workflows/*.md`
- `Governance/*.md`
- `Views/*.canvas`
- `System/ledger.json`
- `System/index.md`

This demo:

- seeds a small in-memory Kogwistar engine
- performs a full Obsidian vault dump
- applies one incremental streamed update into the same vault

If you want a static snapshot instead of the in-memory demo, use:

```bash
kogwistar-obsidian-sink build-from-export --input examples/sample_graph_export.json --vault ./demo_vault
```

## Verification

The repo is meant to be verified in two layers:

1. **Automated file-level tests** with `pytest`
2. **One manual Obsidian smoke check** against the generated vault

That is enough for this project because Obsidian consumes the generated files directly. We do not need a Python adapter inside Obsidian itself.

### What `pytest` covers

- deterministic note generation
- stable `kg_id -> file path` mapping
- event-consumer-to-vault flow
- safe round-trip parsing for explicitly editable sections
- canvas and index file generation

### Manual Obsidian smoke check

1. Build the vault:
   ```bash
   kogwistar-obsidian-sink build-from-export --input examples/sample_graph_export.json --vault ./demo_vault
   ```
2. Open Obsidian.
3. Choose **Open folder as vault**.
4. Select `./demo_vault`.
5. Confirm these open normally:
   - `System/index.md`
   - at least one note under `Concepts/`, `Documents/`, or `Projects/`
   - at least one `.canvas` file under `Views/`
6. Edit only the `## User Notes` block in a note.
7. Rebuild the vault from the same input.
8. Confirm generated sections are still deterministic and your user notes remain in the editable block.

## Using with Kogwistar

This repo avoids depending on unstable internals too aggressively. Instead, it supports two practical integration shapes:

### Shape A: export-first integration

Use a Kogwistar-side script or workflow step to produce a JSON export of entity-like payloads:

```json
{
  "entities": [
    {
      "id": "node:concept:hypergraph-rag",
      "label": "Hypergraph RAG",
      "type": "concept",
      "summary": "Graph-native retrieval idea",
      "metadata": {"tags": ["concept", "retrieval"]},
      "mentions": []
    }
  ]
}
```

Then run:

```bash
kogwistar-obsidian-sink build-from-export --input path/to/export.json --vault /path/to/vault
```

### Shape B: in-process adapter

If `kogwistar` is installed in the same environment, you can adapt live entity objects using duck typing:

```python
from kogwistar_obsidian_sink.integrations.kogwistar_adapter import KogwistarDuckProvider
from kogwistar_obsidian_sink.sinks.obsidian import ObsidianVaultSink

provider = KogwistarDuckProvider(entities=my_entities)
sink = ObsidianVaultSink(vault_root="./vault")
sink.build(provider)
```

This works with Kogwistar-like objects that expose fields such as `id`, `label`, `summary`, `metadata`, `mentions`, `source_ids`, `target_ids`, `relation`, or `type`.

### Shape C: in-memory Kogwistar engine

This is the quickest way to show a real KG pipeline without a persistent backend. The sink does not need embeddings directly, so a 1-dim embedder is enough for the demo engine. The packaged demo command below runs this full path for you:

```bash
kogwistar-obsidian-sink build-in-memory-demo --vault ./demo_vault
```

```python
from kogwistar_obsidian_sink.demo.in_memory_obsidian_demo import run_end_to_end_demo

run_end_to_end_demo("./demo_vault")
```

For the smallest possible end-to-end proof, use the packaged demo command above.

If you want a static export instead, use the existing sample export:

```bash
kogwistar-obsidian-sink build-from-export --input examples/sample_graph_export.json --vault ./demo_vault
```

## CDC scaffold

A minimal event consumer is included for deeper integration. This is the incremental streaming path and is the preferred default for ongoing updates:

```bash
kogwistar-obsidian-sink consume-events   --events examples/sample_events.jsonl   --vault ./demo_vault
```

This is intentionally conservative. In a real deployment, the consumer should subscribe to an authoritative projection event stream and then refresh changed entities from Kogwistar before materializing files.

For a one-shot full refresh, use `build-from-export` instead. That mode materializes the entire vault from a snapshot export in one pass.

The streaming path keeps an inbox log at `System/inbox.jsonl` and a materialized state file at `System/materialized_state.json`, so incremental runs can resume cleanly after a full rebuild.

### Dump and stream example

1. Start with a full dump:
   ```bash
   kogwistar-obsidian-sink build-from-export --input examples/sample_graph_export.json --vault ./demo_vault
   ```
2. Then stream new changes:
   ```bash
   kogwistar-obsidian-sink consume-events --events examples/sample_events.jsonl --vault ./demo_vault
   ```
3. Re-run the stream command as new events arrive. The inbox/state files let the sink apply only impacted note changes instead of redumping everything.

## Tutorial

### End-to-end local tutorial

1. Install this repo.
2. Build the demo vault:
   ```bash
   kogwistar-obsidian-sink build-from-export --input examples/sample_graph_export.json --vault ./demo_vault
   ```
3. Open Obsidian.
4. Choose **Open folder as vault**.
5. Select `./demo_vault`.
6. In Obsidian, browse:
   - `System/index.md`
   - `Views/*.canvas`
   - entity notes under `Concepts/`, `Documents/`, `Projects/`
7. Edit only the `## User Notes` section in any mixed-mode note.
8. Run the round-trip inspector:
   ```bash
   kogwistar-obsidian-sink inspect-note --path ./demo_vault/Concepts/Hypergraph RAG.md
   ```
9. Rebuild from source again:
   ```bash
   kogwistar-obsidian-sink build-from-export --input examples/sample_graph_export.json --vault ./demo_vault
   ```
10. Confirm that generated sections remain deterministic while user notes are preserved only if you later wire in a merge policy.

### How to link the vault to Obsidian cleanly

#### Option 1: use the built vault directly

Point Obsidian at the generated vault folder. This is simplest and works well when the vault is projection-only.

#### Option 2: keep a dedicated sink workspace

Recommended structure:

```text
my-knowledge/
  authoritative/      # Kogwistar repos, exports, runtime
  vaults/
    obsidian-main/    # generated sink output
```

Then open `vaults/obsidian-main` as an Obsidian vault.

#### Option 3: symlink into an existing vault

If you already have a larger Obsidian vault, symlink a generated subtree into it.

Example on Linux/macOS:

```bash
ln -s /absolute/path/generated_vault/Concepts ~/Documents/MyVault/Kogwistar/Concepts
```

Example on Windows PowerShell:

```powershell
New-Item -ItemType SymbolicLink -Path "$HOME\Documents\MyVault\Kogwistar\Concepts" -Target "C:\absolute\path\generated_vault\Concepts"
```

Keep generated folders separate from heavily hand-edited notes.

## Development

```bash
pip install -e .[dev]
pytest -q
```
