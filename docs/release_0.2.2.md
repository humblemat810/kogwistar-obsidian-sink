# Kogwistar Obsidian Sink 0.2.2

## Release scope

- Align the sink runtime dependency with `kogwistar==0.6.4` for CPython 3.12
  and newer.
- Narrow dynamic payload boundaries and add explicit render-result typing without
  changing the exported vault format.
- Keep the Python 3.11 and PyPy 3.11 compatibility path free of the optional
  Kogwistar runtime dependency.
- Retain the existing CPython 3.12-3.14 and PyPy 3.11 CI coverage.

## Verification

The release branch must pass the complete sink test matrix, Ruff checks,
package-build checks, and `git diff --check` before tag `v0.2.2` is created.

This package remains a one-way projection sink. The separate
`kogwistar-obsidian-source` package owns managed-vault import and reversible
synchronization.
