//! Native tree projection: hierarchy is structural, never whitespace in a label.
use peer_core::FileEntry;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub(super) struct Row {
    pub path: String,
    pub label: String,
    pub depth: usize,
    pub file: Option<FileEntry>,
}
impl Row {
    pub fn key(&self) -> String {
        format!(
            "{}:{}",
            if self.file.is_some() { "file" } else { "dir" },
            self.path
        )
    }
}
#[derive(Default)]
struct Directory {
    dirs: BTreeMap<String, Directory>,
    files: Vec<FileEntry>,
}
pub(super) fn reveal(path: &str, collapsed: &mut BTreeSet<String>) {
    for (i, _) in path.match_indices('/') {
        collapsed.remove(&path[..i]);
    }
}
pub(super) fn rows(files: &[FileEntry], filter: &str, collapsed: &BTreeSet<String>) -> Vec<Row> {
    let query = filter.to_lowercase();
    if !query.is_empty() {
        return files
            .iter()
            .filter(|f| f.path.to_lowercase().contains(&query))
            .map(|f| Row {
                path: f.path.clone(),
                label: f.path.clone(),
                depth: 0,
                file: Some(f.clone()),
            })
            .collect();
    }
    let mut root = Directory::default();
    for f in files {
        let mut dir = &mut root;
        let parts: Vec<_> = f.path.split('/').collect();
        for name in &parts[..parts.len().saturating_sub(1)] {
            dir = dir.dirs.entry((*name).into()).or_default();
        }
        dir.files.push(f.clone());
    }
    fn flatten(
        dir: Directory,
        prefix: &str,
        depth: usize,
        closed: &BTreeSet<String>,
        out: &mut Vec<Row>,
    ) {
        for (name, child) in dir.dirs {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            out.push(Row {
                path: path.clone(),
                label: name,
                depth,
                file: None,
            });
            if !closed.contains(&path) {
                flatten(child, &path, depth + 1, closed, out);
            }
        }
        let mut files = dir.files;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        for file in files {
            out.push(Row {
                path: file.path.clone(),
                label: file.path.rsplit('/').next().unwrap_or(&file.path).into(),
                depth,
                file: Some(file),
            });
        }
    }
    let mut out = Vec::new();
    flatten(root, "", 0, collapsed, &mut out);
    out
}
pub(super) fn status(status: &str) -> &'static str {
    match status {
        "added" => "A",
        "untracked" => "U",
        "deleted" => "D",
        "modified" => "M",
        "renamed" => "R",
        "conflict" => "!",
        _ => "",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn file(path: &str) -> FileEntry {
        serde_json::from_value(serde_json::json!({"id":path,"path":path,"status":"modified","oldPath":null,"additions":0,"deletions":0,"kind":"text","size":0,"deferred":false})).unwrap()
    }
    #[test]
    fn hierarchy_filter_and_reveal_preserve_collapsed_state() {
        let files = vec![
            file("src/z.rs"),
            file("README.md"),
            file("src/deep/a.rs"),
            file("other/a.rs"),
        ];
        let mut closed = BTreeSet::from(["src".into(), "src/deep".into()]);
        let tree = rows(&files, "", &closed);
        assert_eq!(
            tree.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            ["other", "other/a.rs", "src", "README.md"]
        );
        let results = rows(&files, "A.RS", &closed);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.depth == 0 && r.label == r.path));
        assert_eq!(closed.len(), 2);
        reveal("src/deep/a.rs", &mut closed);
        assert!(closed.is_empty());
        assert_eq!(
            rows(&files, "", &closed)
                .iter()
                .find(|r| r.path == "src/deep/a.rs")
                .unwrap()
                .depth,
            2
        );
    }
    #[test]
    fn file_to_directory_change_keeps_distinct_row_identity() {
        let files = vec![file("swap"), file("swap/child.rs")];
        let all = rows(&files, "", &BTreeSet::new());
        assert_eq!(all.iter().map(Row::key).collect::<BTreeSet<_>>().len(), 3);
        let closed = rows(&files, "", &BTreeSet::from(["swap".into()]));
        assert_eq!(closed.len(), 2);
        assert!(closed[0].file.is_none());
        assert!(closed[1].file.is_some());
    }
    #[test]
    fn highlight_keeps_original_unicode_boundaries() {
        assert_eq!(matches("İ/猫.rs", "猫"), vec![3..6]);
        assert_eq!(matches("İ/猫.rs", "i"), vec![0..2]);
    }
    #[test]
    fn large_tree_has_unique_rows_and_exact_deep_paths() {
        let files: Vec<_> = (0..10000)
            .map(|i| file(&format!("deep/one/two/three/file{i:05}.rs")))
            .collect();
        let tree = rows(&files, "", &BTreeSet::new());
        assert_eq!(tree.len(), 10004);
        assert_eq!(tree.last().unwrap().path, "deep/one/two/three/file09999.rs");
        assert_eq!(
            tree.iter().map(|r| &r.path).collect::<BTreeSet<_>>().len(),
            tree.len()
        );
    }
}

/// Match case-insensitively while retaining original UTF-8 boundaries (İ expands).
pub(super) fn matches(text: &str, query: &str) -> Vec<std::ops::Range<usize>> {
    if query.is_empty() {
        return vec![];
    }
    let mut folded = String::new();
    let mut map = Vec::new();
    for (start, ch) in text.char_indices() {
        let lower = ch.to_lowercase().to_string();
        map.extend(std::iter::repeat_n(
            (start, start + ch.len_utf8()),
            lower.len(),
        ));
        folded.push_str(&lower);
    }
    let query = query.to_lowercase();
    folded
        .match_indices(&query)
        .map(|(i, found)| map[i].0..map[i + found.len() - 1].1)
        .collect()
}
