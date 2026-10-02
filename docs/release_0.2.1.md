# kogwistar-obsidian-sink 0.2.1

Released as tag `v0.2.1`.

## Highlights

- Aligns the sink with the released Kogwistar 0.6 API.
- Preserves multimodal grounding and source-reference compatibility in sink
  serialization and event handling.
- Refreshes the lockfile and release metadata for the published package.
- Adds the gated PyPI release workflow and keeps the package installable without
  adding an MCP runtime dependency.

## Compatibility

- Kogwistar: compatible with the 0.6 release line used by this release.
- The sink does not embed an MCP server and does not require MCP at runtime.
- PyPy 3.11 support is covered by the repository CI contract; native behavior
  remains subject to the installed Kogwistar backend and its dependencies.

## Validation

The release was validated by the sink test suite, multimodal grounding tests,
packaging checks, dependency lock verification, and the release workflow.
Downstream applications should pin `kogwistar-obsidian-sink==0.2.1` for a
reproducible install.

