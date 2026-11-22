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
    let Some(rest) = text.strip_prefix("---\n") else {
        return text.to_string();
    };
    let Some(end) = rest.find("\n---\n") else {
        return text.to_string();
    };
    rest[end + 5..].trim_start_matches('\n').to_string()
}

fn unquote(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex_byte = if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && bytes[index + 1..index + 3]
                .iter()
                .all(u8::is_ascii_hexdigit)
        {
            Some(
                u8::from_str_radix(
                    std::str::from_utf8(&bytes[index + 1..index + 3]).expect("hex"),
                    16,
                )
                .expect("valid hex"),
            )
        } else {
            None
        };
        match hex_byte {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

pub fn normalize_target(raw: &str) -> String {
    let mut target = unquote(raw.trim());
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

    #[test]
    fn decodes_markdown_destinations() {
        assert_eq!(
            extract_internal_link_targets("[Example](My%20Note.md)"),
            vec!["My Note".to_string()]
        );
    }

    #[test]
    fn ignores_external_links_and_embedded_images() {
        assert!(extract_internal_link_targets(
            "![Alt](image.png) [Web](https://example.com) [Mail](mailto:test@example.com)"
        )
        .is_empty());
    }

    #[test]
    fn normalizes_fragments() {
        assert_eq!(
            extract_internal_link_targets("[Example](B# A%20B)"),
            vec!["B#A B".to_string()]
        );
    }
}
