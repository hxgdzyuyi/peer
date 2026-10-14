//! UI-independent source comparison and coordinate handling. Never compares rendered text.
use crate::FileContent;
use similar::{Algorithm, TextDiff};
use std::time::Duration;

/// Git's quoted-path syntax uses octal escapes for control bytes, preserving arbitrary UTF-8.
fn patch_path(prefix: &str, path: &str) -> String {
    let mut result = String::from("\"");
    result.push_str(prefix);
    for byte in path.bytes() {
        match byte {
            b'"' => result.push_str("\\\""),
            b'\\' => result.push_str("\\\\"),
            32..=126 => result.push(byte as char),
            _ => result.push_str(&format!("\\{byte:03o}")),
        }
    }
    result.push('"');
    result
}

/// Supply all source context so expansion and line jumps are never invented by the renderer.
/// The caller must run this CPU work off the UI thread. Similar's deadline bounds the
/// expensive matching phase; its fallback still produces a correct (less minimal) patch.
pub fn unified_patch(file: &FileContent) -> String {
    let old = file.old_content.as_deref().unwrap_or("");
    let new = file.new_content.as_deref().unwrap_or("");
    let old_path = if file.old_content.is_none() {
        "/dev/null".into()
    } else {
        patch_path("a/", file.old_path.as_deref().unwrap_or(&file.path))
    };
    let new_path = if file.new_content.is_none() {
        "/dev/null".into()
    } else {
        patch_path("b/", &file.path)
    };
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .timeout(Duration::from_secs(2))
        .diff_lines(old, new);
    diff.unified_diff()
        .context_radius(old.lines().count().max(new.lines().count()))
        .header(&old_path, &new_path)
        .to_string()
}

/// Convert an original UTF-8 byte offset to LSP UTF-16. Tabs count as one source
/// character, astral characters as two; a CRLF is not part of a line's column.
pub fn utf16_column(line: &str, byte: usize) -> Result<u32, String> {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    if byte > line.len() || !line.is_char_boundary(byte) {
        return Err("Column is not a source character boundary".into());
    }
    u32::try_from(line[..byte].encode_utf16().count()).map_err(|_| "Column too large".into())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub side: &'static str,
    pub line: u32,
    pub byte: usize,
    pub end_byte: usize,
    pub utf16: u32,
}

/// Regex is escaped before use: queries are literal, Unicode case-insensitive.
pub fn search(file: &FileContent, query: &str) -> Vec<SearchHit> {
    if query.is_empty() {
        return vec![];
    }
    let Ok(pattern) = regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()
    else {
        return vec![];
    };
    let mut hits = Vec::new();
    for (side, source) in [("old", &file.old_content), ("new", &file.new_content)] {
        if let Some(source) = source {
            for (line, text) in source.split_inclusive('\n').enumerate() {
                for hit in pattern.find_iter(text.trim_end_matches(['\r', '\n'])) {
                    if let Ok(utf16) = utf16_column(text, hit.start()) {
                        hits.push(SearchHit {
                            side,
                            line: line as u32 + 1,
                            byte: hit.start(),
                            end_byte: hit.end(),
                            utf16,
                        });
                    }
                    if hits.len() >= 100_000 {
                        return hits;
                    }
                }
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    fn content(old: Option<&str>, new: Option<&str>) -> FileContent {
        FileContent {
            snapshot_id: "s".into(),
            file_id: "f".into(),
            path: "中\tfile.rs".into(),
            old_path: None,
            status: "modified".into(),
            kind: "text".into(),
            old_content: old.map(str::to_owned),
            new_content: new.map(str::to_owned),
            fingerprint: "x".into(),
            tab_width: 4,
            deferred: false,
            size: 0,
            metadata: String::new(),
        }
    }
    #[test]
    fn crlf_and_unterminated_content_are_preserved() {
        let patch = unified_patch(&content(Some("same\r\nold"), Some("same\r\nnew")));
        assert!(patch.contains(" same\r\n"));
        assert!(patch.contains("-old\n\\ No newline at end of file"));
        assert!(patch.contains("+new\n\\ No newline at end of file"));
        assert!(patch.contains("\\344\\270\\255\\011file.rs"));
    }
    #[test]
    fn missing_sides_and_complete_context() {
        assert!(unified_patch(&content(None, Some("new\n"))).starts_with("--- /dev/null\n"));
        let old = (0..100).map(|n| format!("{n}\n")).collect::<String>();
        let new = old.replace("50\n", "changed\n");
        let patch = unified_patch(&content(Some(&old), Some(&new)));
        assert!(patch.contains("@@ -1,100 +1,100 @@"));
        assert!(patch.contains(" 99\n"));
    }
    #[test]
    fn utf16_is_not_a_visual_column() {
        assert_eq!(utf16_column("\t中😀x\r\n", 8), Ok(4));
        assert!(utf16_column("😀", 1).is_err());
        assert!(utf16_column("x\r\n", 2).is_err());
        let hits = search(&content(None, Some("\t中😀Ab.C\r\n")), "ab.c");
        assert_eq!(
            hits,
            vec![SearchHit {
                side: "new",
                line: 1,
                byte: 8,
                end_byte: 12,
                utf16: 4
            }]
        );
    }
}
