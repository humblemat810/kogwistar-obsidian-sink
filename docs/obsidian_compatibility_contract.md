# Obsidian Compatibility Contract

This repository projects Kogwistar entities into a plain Obsidian vault.
The output must remain readable without plugins and must preserve Obsidian-native graph behavior.

## Core Rules

- Node = one Markdown file.
- Edge = one internal link in file content.
- Preferred edge syntax = wikilink.
- Canonical filenames must be stable across reruns.
- Filenames must be sanitized for filesystem safety.
- One canonical note file should exist per entity.
- Semantic metadata may live in YAML or prose, but Graph View only sees internal links.
- YAML relationship fields do not create graph edges by themselves.
- Multiple semantic relationships between the same pair of nodes must be preserved as distinct markdown records, even if Graph View later visually collapses them.
- Vault output must not depend on symlinks, `.lnk` files, or plugin-specific behavior.

## Linking Rules

- Use wikilinks for note-to-note links by default.
- Use path-qualified wikilinks when titles could collide.
- Preserve heading fragments like `[[Note#Heading]]`.
- Preserve block fragments like `[[Note#^block-id]]` when present.
- Allow alias rendering as `[[Canonical Note|Alias]]` when the display text differs.
- Keep attachment links explicit and extension-preserving.

## Graph View Expectations

- Internal links must point to vault-relative targets.
- Excluded files must not be projected into the vault.
- Dangling links may be emitted intentionally, but they should be logged or otherwise accounted for.

## Projection Discipline

- Kogwistar remains authoritative.
- The Obsidian vault is a rebuildable projection.
- Generated structure should not be inferred back into graph mutations unless a dedicated write-back path exists.
- Plain Obsidian should be able to browse, rename, and backlink notes without any adapter plugin.
