# Kogwistar Obsidian Sink 0.2.3

## Release scope

- Update the CPython 3.12+ runtime dependency to `kogwistar==0.7.0`.
- Preserve the Python 3.11 and PyPy 3.11 paths without the optional Kogwistar dependency.
- No exported vault format or sink behavior changes.

## Verification

- Sink unit and demo tests pass against the Kogwistar 0.7.0 source compatibility path.
- Poetry lock, package metadata, Ruff, and package build checks pass before tagging `v0.2.3`.

This package remains a one-way projection sink. The separate
`kogwistar-obsidian-source` package owns managed-vault import and reversible
synchronization.
