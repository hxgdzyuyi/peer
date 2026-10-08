use peer_core::{repository, theme, Comparison, Inspector, NewComment};
use std::{fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(
        dir.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    git(dir.path(), &["config", "core.autocrlf", "false"]);
    fs::write(
        dir.path().join("hello.ts"),
        "const oldValue = 1;\n\tconsole.log(oldValue);\n",
    )
    .unwrap();
    fs::write(dir.path().join("keep.txt"), "unchanged\n").unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored/\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "initial"]);
    dir
}
fn mode(mode: &str) -> Comparison {
    Comparison {
        mode: mode.into(),
        ..Comparison::default()
    }
}
fn code() -> Comparison {
    Comparison {
        view: "code".into(),
        ..Comparison::default()
    }
}

#[tokio::test]
async fn staged_unstaged_and_combined_are_distinct() {
    let repo = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    fs::write(repo.path().join("hello.ts"), "staged\n").unwrap();
    git(repo.path(), &["add", "hello.ts"]);
    fs::write(repo.path().join("hello.ts"), "working\n").unwrap();
    fs::write(repo.path().join("new.ts"), "new\n").unwrap();
    for (selection, old, new, untracked) in [
        ("all", "const oldValue", "working", true),
        ("staged", "const oldValue", "staged", false),
        ("unstaged", "staged", "working", true),
    ] {
        let s = app
            .open_repository(repo.path().to_str().unwrap(), Some(mode(selection)))
            .await
            .unwrap();
        assert_eq!(s.files.iter().any(|f| f.path == "new.ts"), untracked);
        let f = s.files.iter().find(|f| f.path == "hello.ts").unwrap();
        let content = app.read_file(&s.id, &f.id, false).await.unwrap();
        assert!(content.old_content.unwrap().starts_with(old));
        assert!(content.new_content.unwrap().starts_with(new));
    }
}

#[tokio::test]
async fn comments_are_anchored_isolated_persistent_and_outdated() {
    let repo = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    fs::write(repo.path().join("hello.ts"), "newValue\r\n\tkept\r\n").unwrap();
    let s = app
        .open_repository(repo.path().to_str().unwrap(), Some(mode("all")))
        .await
        .unwrap();
    let f = s.files.iter().find(|f| f.path == "hello.ts").unwrap();
    let content = app.read_file(&s.id, &f.id, false).await.unwrap();
    let comment = app
        .add_comment(NewComment {
            snapshot_id: s.id.clone(),
            file_id: f.id.clone(),
            fingerprint: content.fingerprint.clone(),
            body: "Why remove this?".into(),
            side: Some("old".into()),
            line: Some(1),
        })
        .await
        .unwrap();
    assert_eq!(
        comment.anchor.line_text.as_deref(),
        Some("const oldValue = 1;")
    );
    app.set_reviewed(&s.id, &f.id, &content.fingerprint, true)
        .await
        .unwrap();
    let cs = app
        .open_repository(repo.path().to_str().unwrap(), Some(code()))
        .await
        .unwrap();
    assert!(app.queue(&cs.id).await.unwrap().comments.is_empty());
    assert!(app
        .add_comment(NewComment {
            snapshot_id: s.id.clone(),
            file_id: f.id.clone(),
            fingerprint: content.fingerprint.clone(),
            body: "bad".into(),
            side: Some("new".into()),
            line: Some(0)
        })
        .await
        .is_err());
    fs::write(repo.path().join("hello.ts"), "changed again\n").unwrap();
    assert!(app.read_file(&s.id, &f.id, false).await.is_err());
    let s2 = app
        .open_repository(repo.path().to_str().unwrap(), Some(mode("all")))
        .await
        .unwrap();
    let queue = app.queue(&s2.id).await.unwrap();
    assert!(queue.comments[0].outdated);
    assert!(queue.reviewed.is_empty());
    let exported: serde_json::Value =
        serde_json::from_str(&app.export(&s2.id).await.unwrap()).unwrap();
    assert_eq!(exported["schemaVersion"], 1);
    assert_eq!(exported["comments"][0]["anchor"]["side"], "old");
    let fresh_app = Inspector::new(store.path()).unwrap();
    let restored = fresh_app
        .open_repository(repo.path().to_str().unwrap(), Some(mode("all")))
        .await
        .unwrap();
    assert_eq!(
        fresh_app.queue(&restored.id).await.unwrap().comments.len(),
        1
    );
    fresh_app.delete_comments(&restored.queue_id, None).unwrap();
    assert!(fresh_app
        .queue(&restored.id)
        .await
        .unwrap()
        .comments
        .is_empty());
}

#[tokio::test]
async fn branch_comparison_uses_merge_base_and_local_edits() {
    let repo = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    git(repo.path(), &["checkout", "-b", "feature"]);
    fs::write(repo.path().join("feature.txt"), "committed\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "feature"]);
    git(repo.path(), &["checkout", "main"]);
    fs::write(repo.path().join("main-only.txt"), "main\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "main advances"]);
    git(repo.path(), &["checkout", "feature"]);
    fs::write(repo.path().join("feature.txt"), "local\n").unwrap();
    let s = app
        .open_repository(
            repo.path().to_str().unwrap(),
            Some(Comparison {
                base_ref: Some("main".into()),
                ..mode("branch")
            }),
        )
        .await
        .unwrap();
    assert_eq!(s.files.len(), 1);
    assert_eq!(s.files[0].path, "feature.txt");
    assert_eq!(
        app.read_file(&s.id, &s.files[0].id, false)
            .await
            .unwrap()
            .new_content
            .as_deref(),
        Some("local\n")
    );
}

#[tokio::test]
async fn code_inventory_renames_binary_and_unborn_repository() {
    let repo = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    fs::create_dir(repo.path().join("ignored")).unwrap();
    fs::write(repo.path().join("ignored/hidden"), "hidden").unwrap();
    git(repo.path(), &["mv", "hello.ts", "renamed 日本語.ts"]);
    fs::write(repo.path().join("binary.dat"), [0u8, 255, 1]).unwrap();
    let s = app
        .open_repository(repo.path().to_str().unwrap(), Some(mode("all")))
        .await
        .unwrap();
    let renamed = s.files.iter().find(|f| f.status == "renamed").unwrap();
    assert_eq!(renamed.old_path.as_deref(), Some("hello.ts"));
    let binary = s.files.iter().find(|f| f.path == "binary.dat").unwrap();
    assert_eq!(
        app.read_file(&s.id, &binary.id, false).await.unwrap().kind,
        "binary"
    );
    let cs = app
        .open_repository(repo.path().to_str().unwrap(), Some(code()))
        .await
        .unwrap();
    assert!(cs.files.iter().any(|f| f.path == "keep.txt"));
    assert!(!cs.files.iter().any(|f| f.path.starts_with("ignored/")));
    let unborn = tempfile::tempdir().unwrap();
    git(unborn.path(), &["init", "-b", "main"]);
    fs::write(unborn.path().join("first.txt"), "initial\n").unwrap();
    git(unborn.path(), &["add", "."]);
    fs::write(unborn.path().join("first.txt"), "edited\n").unwrap();
    let s = app
        .open_repository(unborn.path().to_str().unwrap(), Some(mode("all")))
        .await
        .unwrap();
    let content = app.read_file(&s.id, &s.files[0].id, false).await.unwrap();
    assert!(content.old_content.is_none());
    assert_eq!(content.new_content.as_deref(), Some("edited\n"));
}

#[test]
fn editorconfig_and_theme_normalization() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join(".editorconfig"),
        "root=true\n[*]\nindent_size=2\n[*.ts]\ntab_width=8\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("src/.editorconfig"),
        "[*.ts]\ntab_width=3\n",
    )
    .unwrap();
    assert_eq!(repository::editor_tab_width(repo.path(), "src/a.ts", 4), 3);
    assert_eq!(repository::editor_tab_width(repo.path(), "a.ts", 4), 8);
    assert_eq!(repository::editor_tab_width(repo.path(), "a.txt", 4), 2);
    let json = "{ // theme\n \"name\": \"My theme\", \"type\": \"dark\", \"colors\": {\"editor.background\": \"#111111\",}, \"tokenColors\": [] }";
    let a = theme::import_theme(json, "fallback").unwrap();
    let b = theme::import_theme(&json.replace("#111111", "#222222"), "fallback").unwrap();
    assert_eq!(a.name, b.name);
    assert_ne!(a.id, b.id);
    assert_eq!(a.definition["name"], a.id);
    assert!(theme::import_theme("{\"include\":\"../theme.json\"}", "bad").is_err());
    assert!(theme::import_theme("{\"tokenColors\":\"other.tmTheme\"}", "bad").is_err());
    assert!(theme::import_theme(
        "{\"colors\": {\"editor.background\": \"url(evil)\"}}",
        "bad"
    )
    .is_err());
    assert!(theme::import_theme("[]", "bad").is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn symlinks_and_unusual_names_remain_data() {
    let repo = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", repo.path().join("external")).unwrap();
    let weird = "spaces \"and\"\tnewline\n.txt";
    fs::write(
        repo.path().join(weird),
        "unchanged whitespace\t \nlast line",
    )
    .unwrap();
    git(repo.path(), &["add", "--", weird]);
    let s = app
        .open_repository(repo.path().to_str().unwrap(), Some(mode("all")))
        .await
        .unwrap();
    assert!(s.files.iter().any(|f| f.path == weird));
    let symlink = s.files.iter().find(|f| f.path == "external").unwrap();
    let data = app.read_file(&s.id, &symlink.id, false).await.unwrap();
    assert_eq!(data.kind, "symlink");
    assert_eq!(data.new_content.as_deref(), Some("/etc/passwd"));
    assert!(repository::safe_path(repo.path(), "../secret").is_err());
}

#[tokio::test]
async fn removed_file_comments_remain_exportable_without_line_coordinates() {
    let repo = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    let s = app
        .open_repository(repo.path().to_str().unwrap(), Some(code()))
        .await
        .unwrap();
    let file = s.files.iter().find(|f| f.path == "keep.txt").unwrap();
    let content = app.read_file(&s.id, &file.id, false).await.unwrap();
    app.add_comment(NewComment {
        snapshot_id: s.id,
        file_id: file.id.clone(),
        fingerprint: content.fingerprint,
        body: "Explain this file".into(),
        side: None,
        line: None,
    })
    .await
    .unwrap();
    fs::remove_file(repo.path().join("keep.txt")).unwrap();
    let s = app
        .open_repository(repo.path().to_str().unwrap(), Some(code()))
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&app.export(&s.id).await.unwrap()).unwrap();
    let comment = &value["comments"][0];
    assert_eq!(comment["outdated"], true);
    assert!(comment["anchor"].get("line").is_none());
    assert!(comment["anchor"].get("side").is_none());
}

#[tokio::test]
async fn conflicts_and_large_files_have_safe_summaries() {
    let repo = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    git(repo.path(), &["checkout", "-b", "other"]);
    fs::write(repo.path().join("hello.ts"), "other\n").unwrap();
    git(repo.path(), &["commit", "-am", "other"]);
    git(repo.path(), &["checkout", "main"]);
    fs::write(repo.path().join("hello.ts"), "main\n").unwrap();
    git(repo.path(), &["commit", "-am", "main"]);
    let merge = Command::new("git")
        .args(["merge", "other"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(!merge.status.success());
    fs::write(repo.path().join("huge.txt"), "line\n".repeat(20_001)).unwrap();
    let s = app
        .open_repository(repo.path().to_str().unwrap(), Some(mode("all")))
        .await
        .unwrap();
    let file = s.files.iter().find(|f| f.path == "hello.ts").unwrap();
    let c = app.read_file(&s.id, &file.id, false).await.unwrap();
    assert_eq!(c.kind, "conflict");
    app.add_comment(NewComment {
        snapshot_id: s.id.clone(),
        file_id: file.id.clone(),
        fingerprint: c.fingerprint,
        body: "Resolve both implementations".into(),
        side: None,
        line: None,
    })
    .await
    .unwrap();
    let large = s.files.iter().find(|f| f.path == "huge.txt").unwrap();
    assert!(
        app.read_file(&s.id, &large.id, false)
            .await
            .unwrap()
            .deferred
    );
    assert!(
        !app.read_file(&s.id, &large.id, true)
            .await
            .unwrap()
            .deferred
    );
}

#[tokio::test]
async fn branch_and_worktree_queue_identity_isolated() {
    let repo = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    let main = app
        .open_repository(repo.path().to_str().unwrap(), Some(code()))
        .await
        .unwrap();
    git(repo.path(), &["checkout", "-b", "other"]);
    let other = app
        .open_repository(repo.path().to_str().unwrap(), Some(code()))
        .await
        .unwrap();
    assert_ne!(main.queue_id, other.queue_id);
    let worktree = tempfile::tempdir().unwrap();
    let path = worktree.path().join("checkout");
    git(
        repo.path(),
        &["worktree", "add", path.to_str().unwrap(), "main"],
    );
    let linked = app
        .open_repository(path.to_str().unwrap(), Some(code()))
        .await
        .unwrap();
    assert_ne!(main.queue_id, linked.queue_id);
    assert_eq!(main.repository.branch, linked.repository.branch);
}

#[test]
fn imported_themes_survive_removal_of_source_and_restart() {
    let store = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("theme.jsonc");
    fs::write(
        &path,
        "{\"name\":\"Local\",\"colors\":{\"editor.background\":\"#fff\"},\"tokenColors\":[]}",
    )
    .unwrap();
    let app = Inspector::new(store.path()).unwrap();
    let theme = app.import_theme(path.to_str().unwrap()).unwrap();
    let mut prefs = app.store.read().unwrap().preferences;
    prefs.appearance.light_theme = theme.id.clone();
    app.save_preferences(prefs).unwrap();
    fs::remove_file(path).unwrap();
    drop(app);
    let restored = Inspector::new(store.path()).unwrap();
    assert_eq!(restored.store.read().unwrap().themes[0].id, theme.id);
    restored.remove_theme(&theme.id).unwrap();
    assert_eq!(
        restored
            .store
            .read()
            .unwrap()
            .preferences
            .appearance
            .light_theme,
        "github-light"
    );
}

#[test]
fn preferences_match_typescript_contract() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/contracts/preferences.json")).unwrap();
    let prefs: peer_core::Preferences = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(prefs).unwrap(), expected);
    assert_eq!(
        serde_json::to_value(peer_core::Preferences::default()).unwrap(),
        expected
    );
}

#[test]
fn font_size_defaults_for_existing_stores_and_persists() {
    let mut legacy: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/contracts/preferences.json")).unwrap();
    legacy.as_object_mut().unwrap().remove("codeFontSize");
    let store = tempfile::tempdir().unwrap();
    fs::write(
        store.path().join("state.json"),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1, "preferences": legacy, "queues": {}, "themes": []
        }))
        .unwrap(),
    )
    .unwrap();
    let app = Inspector::new(store.path()).unwrap();
    let mut prefs = app.store.read().unwrap().preferences;
    assert_eq!(prefs.code_font_size, 12);
    prefs.code_font_size = 18;
    app.save_preferences(prefs.clone()).unwrap();
    for invalid in [0, 9, 33, u32::MAX] {
        prefs.code_font_size = invalid;
        assert!(app.save_preferences(prefs.clone()).is_err());
    }
    drop(app);
    let restored = Inspector::new(store.path()).unwrap();
    assert_eq!(
        restored.store.read().unwrap().preferences.code_font_size,
        18
    );
}

#[tokio::test]
async fn code_feedback_does_not_expire_when_identical_content_is_staged() {
    let repo = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    fs::write(repo.path().join("new.ts"), "const value = 1;\n").unwrap();
    let s = app
        .open_repository(repo.path().to_str().unwrap(), Some(code()))
        .await
        .unwrap();
    let f = s.files.iter().find(|f| f.path == "new.ts").unwrap();
    let c = app.read_file(&s.id, &f.id, false).await.unwrap();
    app.add_comment(NewComment {
        snapshot_id: s.id.clone(),
        file_id: f.id.clone(),
        fingerprint: c.fingerprint,
        body: "Question".into(),
        side: None,
        line: Some(1),
    })
    .await
    .unwrap();
    git(repo.path(), &["add", "new.ts"]);
    git(repo.path(), &["commit", "-m", "add file"]);
    let next = app
        .open_repository(repo.path().to_str().unwrap(), Some(code()))
        .await
        .unwrap();
    assert!(!app.queue(&next.id).await.unwrap().comments[0].outdated);
}

#[tokio::test]
async fn submodule_updates_and_deletions_have_file_comment_summaries() {
    let repo = fixture();
    let child = fixture();
    let store = tempfile::tempdir().unwrap();
    let app = Inspector::new(store.path()).unwrap();
    git(
        repo.path(),
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child.path().to_str().unwrap(),
            "module",
        ],
    );
    git(repo.path(), &["commit", "-am", "add submodule"]);
    let module = repo.path().join("module");
    git(&module, &["config", "user.name", "Test"]);
    git(&module, &["config", "user.email", "test@example.invalid"]);
    fs::write(module.join("keep.txt"), "updated\n").unwrap();
    git(&module, &["commit", "-am", "update"]);
    git(repo.path(), &["add", "module"]);
    let s = app
        .open_repository(repo.path().to_str().unwrap(), Some(mode("staged")))
        .await
        .unwrap();
    let f = s.files.iter().find(|f| f.path == "module").unwrap();
    let c = app.read_file(&s.id, &f.id, false).await.unwrap();
    assert_eq!(c.kind, "submodule");
    assert!(c.metadata.contains("Subproject commit"));
    app.add_comment(NewComment {
        snapshot_id: s.id,
        file_id: f.id.clone(),
        fingerprint: c.fingerprint,
        body: "Why update the submodule?".into(),
        side: None,
        line: None,
    })
    .await
    .unwrap();
    git(repo.path(), &["rm", "-f", "module"]);
    let next = app
        .open_repository(repo.path().to_str().unwrap(), Some(mode("all")))
        .await
        .unwrap();
    let f = next.files.iter().find(|f| f.path == "module").unwrap();
    assert_eq!(
        app.read_file(&next.id, &f.id, false).await.unwrap().kind,
        "submodule"
    );
}
