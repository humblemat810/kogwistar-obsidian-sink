use regex::Regex;

pub fn slugify(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_separator = false;
    for character in value.trim().to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            last_was_separator = false;
        } else if !last_was_separator {
            output.push('-');
            last_was_separator = true;
        }
    }
    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "note".to_string()
    } else {
        trimmed
    }
}

pub fn strip_frontmatter(text: &str) -> String {
    if let Some(rest) = text.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return rest[end + 5..].trim_start_matches('\n').to_string();
        }
    }
    text.to_string()
}

pub fn normalize_target(raw: &str) -> String {
    let mut target = raw.trim().to_string();
    if let Some(pipe_index) = target.find('|') {
        target.truncate(pipe_index);
    }
    let target = target.trim();
    let target = if let Some(stripped) = target.strip_prefix("![[") {
        stripped.strip_suffix("]]").unwrap_or(target)
    } else {
        target
    };
    let mut target = target.to_string();
    if target.to_lowercase().ends_with(".md") {
        target.truncate(target.len() - 3);
    }
    if let Some(hash_index) = target.find('#') {
        let base = target[..hash_index].trim().to_string();
        let fragment = target[hash_index + 1..].trim().to_string();
        target = if fragment.is_empty() {
            base
        } else {
            format!("{base}#{fragment}")
        };
    }
    target
}

pub fn extract_internal_link_targets(text: &str) -> Vec<String> {
    let body = strip_frontmatter(text);
    let wiki = Regex::new(r"\[\[([^\]]+)\]\]").expect("wikilink regex");
    let markdown = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").expect("markdown link regex");
    let mut targets = Vec::new();
    for capture in wiki.captures_iter(&body) {
        targets.push(normalize_target(&capture[1]));
    }
    for capture in markdown.captures_iter(&body) {
        if capture
            .get(0)
            .map(|match_| match_.start())
            .unwrap_or_default()
            > 0
            && body.as_bytes()[capture.get(0).unwrap().start() - 1] == b'!'
        {
            continue;
        }
        let destination = capture[2].trim();
        if destination.contains("://") || destination.starts_with("mailto:") {
            continue;
        }
        targets.push(normalize_target(destination));
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_wikilinks_and_markdown_links() {
        assert_eq!(
            extract_internal_link_targets("Body [[B]] and [B](B.md) and [[B|alias text]]"),
            vec!["B".to_string(), "B".to_string(), "B".to_string()]
        );
    }

    #[test]
    fn ignores_frontmatter_aliases() {
        assert!(
            extract_internal_link_targets("---\naliases: [\"B\"]\n---\n\nNo links.\n").is_empty()
        );
    }
}
