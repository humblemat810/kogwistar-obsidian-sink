use anyhow::{bail, Result};
use regex::Regex;
use serde_json::{json, Value};

pub struct SafeNoteEdit {
    pub kg_id: String,
    pub user_notes: String,
}

pub fn parse_safe_note(path: &std::path::Path) -> Result<SafeNoteEdit> {
    let text = std::fs::read_to_string(path)?;
    let kg_id = Regex::new(r#"(?m)^kg_id:\s+"([^"]+)""#).expect("kg id regex");
    let user_notes = Regex::new(r"(?s)<!-- USER-OWNED-START -->(.*?)<!-- USER-OWNED-END -->")
        .expect("user notes regex");
    let Some(kg) = kg_id.captures(&text) else {
        bail!("Missing kg_id in frontmatter");
    };
    let Some(notes) = user_notes.captures(&text) else {
        bail!("Missing user-owned block");
    };
    Ok(SafeNoteEdit {
        kg_id: kg[1].to_string(),
        user_notes: notes[1].trim().to_string(),
    })
}

pub fn inspect_note(path: &std::path::Path) -> Result<Value> {
    let edit = parse_safe_note(path)?;
    Ok(json!({"kg_id": edit.kg_id, "user_notes": edit.user_notes}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_user_notes() {
        let path = std::env::temp_dir().join(format!("kogwistar-safe-note-{}", std::process::id()));
        fs::write(&path, "---\nkg_id: \"node:a\"\n---\n\n<!-- USER-OWNED-START -->\nnotes\n<!-- USER-OWNED-END -->\n").unwrap();
        let value = inspect_note(&path).unwrap();
        assert_eq!(value["kg_id"], "node:a");
        assert_eq!(value["user_notes"], "notes");
        fs::remove_file(&path).unwrap();
    }
}
