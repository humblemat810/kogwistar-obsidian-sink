from __future__ import annotations

import re
from urllib.parse import unquote


FRONTMATTER_RE = re.compile(r"^---\n.*?\n---\n+", re.DOTALL)
WIKILINK_RE = re.compile(r"\[\[([^\]]+)\]\]")
MARKDOWN_LINK_RE = re.compile(r"(?<!!)\[([^\]]+)\]\(([^)]+)\)")


def strip_frontmatter(text: str) -> str:
    return FRONTMATTER_RE.sub("", text, count=1)


def _normalize_target(raw: str) -> str:
    target = unquote(raw.strip())
    if "|" in target:
        target = target.split("|", 1)[0]
    target = target.strip()
    if target.startswith("![[") and target.endswith("]]"):
        target = target[3:-2]
    if target.lower().endswith(".md"):
        target = target[:-3]
    if "#" in target:
        base, fragment = target.split("#", 1)
        target = base.strip()
        fragment = fragment.strip()
        if fragment:
            target = f"{target}#{fragment}"
    return target


def extract_internal_link_targets(text: str) -> list[str]:
    """Extract Obsidian-style internal link targets from markdown content.

    The helper is intentionally small and conservative:
    - YAML frontmatter is ignored
    - wikilinks and relative markdown links are normalized to vault targets
    - markdown destinations are URL-decoded before normalization
    """

    body = strip_frontmatter(text)
    targets: list[str] = []

    for match in WIKILINK_RE.finditer(body):
        raw = match.group(1)
        targets.append(_normalize_target(raw))

    for match in MARKDOWN_LINK_RE.finditer(body):
        dest = match.group(2).strip()
        if "://" in dest or dest.startswith("mailto:"):
            continue
        targets.append(_normalize_target(dest))

    return targets
