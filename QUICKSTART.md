# Quick Start

This repository uses a GitHub-first dependency setup for consumers.
For consumer installs, `pip install -e .` resolves `kogwistar` from GitHub through `pyproject.toml`.

If you are a contributor and want the checked-out local `./kogwistar` subtree to win instead, run the bootstrap script manually.
It is opt-in and does not run automatically during normal installs.

## 1. Create a virtual environment

```bash
python -m venv .venv
source .venv/bin/activate
```

On Windows PowerShell:

```powershell
python -m venv .venv
.\.venv\Scripts\Activate.ps1
```

## 2. Install the repo

```bash
pip install -e .
```

This keeps the default `kogwistar` dependency coming from GitHub.

## 3. Optional: switch to the local Kogwistar subtree

```bash
bash scripts/bootstrap-dev.sh
```

The script:

- clones `./kogwistar` if it is missing
- installs the local subtree editable into the active environment
- leaves the GitHub dependency as the default when you do not run it

Windows users can run the same Bash script from Git Bash or WSL.

## 4. Run the demo

```bash
kogwistar-obsidian-sink build-in-memory-demo --vault ./demo_vault
```

Then open `./demo_vault` in Obsidian as a vault.
