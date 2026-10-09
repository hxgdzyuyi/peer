use peer_core::{
    navigation::{default_servers, NavigationRequest, NavigationResult, Position},
    Comparison, Inspector, Snapshot,
};
use std::{fs, path::Path, process::Command, sync::Arc, time::Duration};

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().into()
}
fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Inspector) {
    let repo = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-b", "main"]);
    git(repo.path(), &["config", "user.name", "Test"]);
    git(
        repo.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    fs::write(repo.path().join(".gitignore"), "node_modules/\n").unwrap();
    fs::write(
        repo.path().join("call.ts"),
        "asdf();\nkeep();\nexternal();\nhang();\ncrash();\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("defs.ts"),
        "export function asdf() { return 'base'; }\n",
    )
    .unwrap();
    fs::write(repo.path().join("keep.ts"), "export const keep = 1;\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "base"]);
    let app = Inspector::new(store.path()).unwrap();
    let mut preferences = app.store.read().unwrap().preferences;
    let mut server = default_servers().remove(1);
    server.executable = "node".into();
    server.args = vec![Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/support/fake-lsp.mjs")
        .to_str()
        .unwrap()
        .into()];
    server.env.insert("PEER_FAKE_FORM".into(), "links".into());
    server.env.insert(
        "PEER_FAKE_LOG".into(),
        store.path().join("servers.log").to_str().unwrap().into(),
    );
    preferences.language_servers = vec![server];
    app.save_preferences(preferences).unwrap();
    (repo, store, app)
}
fn request(
    snapshot: &Snapshot,
    side: &str,
    path: &str,
    line: u32,
    character: u32,
) -> NavigationRequest {
    NavigationRequest {
        request_id: uuid::Uuid::new_v4().to_string(),
        snapshot_id: snapshot.id.clone(),
        side: side.into(),
        path: Some(path.into()),
        document_id: None,
        position: Position { line, character },
        operation: "auto".into(),
    }
}
async fn source(app: &Inspector, result: &NavigationResult) -> String {
    assert_eq!(result.locations.len(), 1, "{result:?}");
    app.read_navigation_file(
        &result.snapshot_id,
        &result.side,
        &result.locations[0].document_id,
        true,
    )
    .await
    .unwrap()
    .content
    .unwrap()
}

#[tokio::test]
async fn navigation_tab_width_preserves_editorconfig_or_uses_the_preference() {
    let (repo, _store, app) = fixture();
    let content = "export function asdf() {\n\treturn 'base';\n}\n";
    fs::write(repo.path().join("defs.ts"), content).unwrap();
    fs::write(repo.path().join(".editorconfig"), "root=true\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "tabbed definition"]);
    let mut preferences = app.store.read().unwrap().preferences;
    preferences.tab_width = 6;
    app.save_preferences(preferences).unwrap();

    for (config, expected) in [
        ("root=true\n", None),
        ("root=true\n[*.ts]\ntab_width=8\n", Some(8)),
        ("root=true\n[*.ts]\ntab_width=1\n", Some(1)),
    ] {
        fs::write(repo.path().join(".editorconfig"), config).unwrap();
        let snapshot = app
            .open_repository(repo.path().to_str().unwrap(), Some(Comparison::default()))
            .await
            .unwrap();
        for (side, expected) in [("old", None), ("new", expected)] {
            let result = app
                .navigate_symbol(request(&snapshot, side, "call.ts", 0, 1))
                .await
                .unwrap();
            let file = app
                .read_navigation_file(&snapshot.id, side, &result.locations[0].document_id, true)
                .await
                .unwrap();
            assert_eq!(file.content.as_deref(), Some(content));
            assert_eq!(file.tab_width, expected, "{side}: {config}");
        }
    }
}

#[tokio::test]
async fn every_comparison_resolves_its_own_complete_repository_version() {
    let (repo, store, app) = fixture();
    git(repo.path(), &["checkout", "-b", "topic"]);
    fs::write(
        repo.path().join("defs.ts"),
        "// head\nexport function asdf() { return 'head'; }\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "head"]);
    fs::write(
        repo.path().join("defs.ts"),
        "// index\n\nexport function asdf() { return 'index'; }\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    fs::write(
        repo.path().join("defs.ts"),
        "// working\n\n\nexport function asdf() { return 'working'; }\n",
    )
    .unwrap();
    let before = git(repo.path(), &["status", "--porcelain=v1"]);
    let index_before = fs::read(repo.path().join(".git/index")).unwrap();
    for (mode, old, new) in [
        ("all", "head", "working"),
        ("staged", "head", "index"),
        ("unstaged", "index", "working"),
        ("branch", "base", "working"),
    ] {
        let snapshot = app
            .open_repository(
                repo.path().to_str().unwrap(),
                Some(Comparison {
                    mode: mode.into(),
                    base_ref: if mode == "branch" {
                        Some("main".into())
                    } else {
                        None
                    },
                    ..Comparison::default()
                }),
            )
            .await
            .unwrap();
        assert!(!snapshot.files.iter().any(|file| file.path == "keep.ts"));
        for (side, expected) in [("old", old), ("new", new)] {
            let result = app
                .navigate_symbol(request(&snapshot, side, "call.ts", 0, 1))
                .await
                .unwrap();
            assert!(source(&app, &result)
                .await
                .contains(&format!("'{expected}'")));
            let definition = &result.locations[0];
            let mut again = request(&snapshot, side, "", 0, 0);
            again.path = None;
            again.document_id = Some(definition.document_id.clone());
            again.position = definition.range.start;
            let references = app.navigate_symbol(again).await.unwrap();
            assert_eq!(references.kind, "references");
            assert_eq!(references.locations[0].path, "call.ts");
            let unchanged = app
                .navigate_symbol(request(&snapshot, side, "call.ts", 1, 1))
                .await
                .unwrap();
            assert_eq!(unchanged.locations[0].path, "keep.ts");
        }
    }
    assert_eq!(before, git(repo.path(), &["status", "--porcelain=v1"]));
    assert_eq!(
        index_before,
        fs::read(repo.path().join(".git/index")).unwrap()
    );
    let roots = fs::read_to_string(store.path().join("servers.log")).unwrap();
    let paths: Vec<_> = roots.lines().collect();
    assert_ne!(paths[paths.len() - 1], paths[paths.len() - 2]);
}

#[tokio::test]
async fn source_is_frozen_and_working_inventory_drift_requires_refresh() {
    let (repo, store, app) = fixture();
    let comparison = Comparison {
        view: "code".into(),
        ..Comparison::default()
    };
    let snapshot = app
        .open_repository(repo.path().to_str().unwrap(), Some(comparison.clone()))
        .await
        .unwrap();
    assert!(
        !store.path().join("servers.log").exists(),
        "ordinary browsing must not start servers"
    );
    fs::write(repo.path().join("keep.ts"), "export const keep = 999;\n").unwrap();
    let error = app
        .navigate_symbol(request(&snapshot, "new", "call.ts", 0, 1))
        .await
        .unwrap_err();
    assert_eq!(error.code, "stale");
    let snapshot = app
        .open_repository(repo.path().to_str().unwrap(), Some(comparison))
        .await
        .unwrap();
    let result = app
        .navigate_symbol(request(&snapshot, "new", "call.ts", 0, 1))
        .await
        .unwrap();
    fs::write(
        repo.path().join("defs.ts"),
        "export const asdf = 'later';\n",
    )
    .unwrap();
    assert!(source(&app, &result).await.contains("'base'"));
    assert!(source(
        &app,
        &app.navigate_symbol(request(&snapshot, "new", "call.ts", 0, 1))
            .await
            .unwrap()
    )
    .await
    .contains("'base'"));
    assert!(app
        .read_navigation_file(&snapshot.id, "new", "/etc/passwd", true)
        .await
        .is_err());
}

#[tokio::test]
async fn cancellation_crash_capabilities_and_external_handles() {
    let (repo, store, app) = fixture();
    let dependency = store.path().join("dependency.ts");
    fs::write(&dependency, "external dependency\n").unwrap();
    let mut preferences = app.store.read().unwrap().preferences;
    preferences.language_servers[0].env.insert(
        "PEER_FAKE_EXTERNAL".into(),
        dependency.to_str().unwrap().into(),
    );
    app.save_preferences(preferences.clone()).unwrap();
    let snapshot = app
        .open_repository(
            repo.path().to_str().unwrap(),
            Some(Comparison {
                view: "code".into(),
                ..Comparison::default()
            }),
        )
        .await
        .unwrap();
    let external = app
        .navigate_symbol(request(&snapshot, "new", "call.ts", 2, 2))
        .await
        .unwrap();
    assert!(external.locations[0].external);
    assert_eq!(external.locations[0].label, "Installed dependency");
    fs::write(dependency, "changed installation\n").unwrap();
    assert_eq!(source(&app, &external).await, "external dependency\n");
    let pending = request(&snapshot, "new", "call.ts", 3, 1);
    let id = pending.request_id.clone();
    let app = Arc::new(app);
    let task_app = app.clone();
    let query = tokio::spawn(async move { task_app.navigate_symbol(pending).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    app.cancel_navigation(&id);
    assert_eq!(query.await.unwrap().unwrap_err().code, "cancelled");
    assert_eq!(
        app.navigate_symbol(request(&snapshot, "new", "call.ts", 4, 1))
            .await
            .unwrap_err()
            .code,
        "serverError"
    );
    assert_eq!(
        app.navigate_symbol(request(&snapshot, "new", "call.ts", 0, 1))
            .await
            .unwrap()
            .locations
            .len(),
        1
    );
    preferences.language_servers[0]
        .env
        .insert("PEER_FAKE_UNSUPPORTED".into(), "1".into());
    app.save_preferences(preferences).unwrap();
    assert_eq!(
        app.navigate_symbol(request(&snapshot, "new", "call.ts", 0, 1))
            .await
            .unwrap_err()
            .code,
        "unsupported"
    );
}

#[tokio::test]
async fn utf16_renames_and_missing_servers() {
    let (repo, _store, app) = fixture();
    git(repo.path(), &["mv", "defs.ts", "renamed.ts"]);
    fs::write(repo.path().join("call.ts"), "'😀'; asdf();\r\n").unwrap();
    let snapshot = app
        .open_repository(repo.path().to_str().unwrap(), Some(Comparison::default()))
        .await
        .unwrap();
    let result = app
        .navigate_symbol(request(&snapshot, "new", "call.ts", 0, 7))
        .await
        .unwrap();
    assert_eq!(result.locations[0].path, "renamed.ts");
    let old = app
        .navigate_symbol(request(&snapshot, "old", "call.ts", 0, 1))
        .await
        .unwrap();
    assert_eq!(old.locations[0].path, "defs.ts");
    let mut preferences = app.store.read().unwrap().preferences;
    preferences.language_servers[0].executable = "peer-no-such-language-server".into();
    app.save_preferences(preferences).unwrap();
    assert_eq!(
        app.navigate_symbol(request(&snapshot, "old", "call.ts", 0, 1))
            .await
            .unwrap_err()
            .code,
        "missingServer"
    );
}

#[tokio::test]
#[ignore = "requires gopls, typescript-language-server and rust-analyzer with their toolchains"]
async fn real_language_servers_resolve_scopes_imports_methods_and_references() {
    for language in ["go", "typescript", "javascript", "rust"] {
        let (repo, _store, app) = fixture();
        let (file, definition, source, marker, method_marker, shadow_marker) = match language {
            "go" => {
                fs::write(
                    repo.path().join("go.mod"),
                    "module example.com/navigation\n\ngo 1.22\n",
                )
                .unwrap();
                fs::write(repo.path().join("defs.go"), "package navigation\nfunc asdf() int { return 1 }\ntype Thing struct{}\nfunc (Thing) method() int { return asdf() }\n").unwrap();
                ("call.go", "defs.go", "package navigation\nfunc use() { y := asdf(); _ = y;\n{ y := 2; _ = y; }; var t Thing; _ = t.method() }\n", "asdf()", "method()", "y := 2; _ = y;")
            }
            "typescript" => {
                fs::write(
                    repo.path().join("tsconfig.json"),
                    "{\"compilerOptions\":{\"strict\":true},\"include\":[\"*.ts\"]}",
                )
                .unwrap();
                fs::write(repo.path().join("defs.ts"), "export function asdf() { return 1; }\nexport class Thing { method() { return asdf(); } }\n").unwrap();
                ("call.ts", "defs.ts", "import { asdf, Thing } from './defs';\nconst y = asdf();\nfunction use() { const y = 2; return y; }\nnew Thing().method();\n", "= asdf()", "method()", "return y;")
            }
            "javascript" => {
                // Do not let TypeScript module resolution prefer the fixture's defs.ts.
                fs::remove_file(repo.path().join("defs.ts")).unwrap();
                fs::write(
                    repo.path().join("jsconfig.json"),
                    "{\"compilerOptions\":{\"checkJs\":true},\"include\":[\"*.js\"]}",
                )
                .unwrap();
                fs::write(repo.path().join("defs.js"), "export function asdf() { return 1; }\nexport class Thing { method() { return asdf(); } }\n").unwrap();
                ("call.js", "defs.js", "import { asdf, Thing } from './defs';\nconst y = asdf();\nfunction use() { const y = 2; return y; }\nnew Thing().method();\n", "= asdf()", "method()", "return y;")
            }
            _ => {
                fs::write(repo.path().join("Cargo.toml"), "[package]\nname=\"nav_smoke\"\nversion=\"0.1.0\"\nedition=\"2021\"\n[lib]\npath=\"lib.rs\"\n").unwrap();
                fs::write(repo.path().join("defs.rs"), "pub fn asdf() -> i32 { 1 }\npub struct Thing;\nimpl Thing { pub fn method(&self) -> i32 { asdf() } }\n").unwrap();
                ("lib.rs", "defs.rs", "mod defs;\nuse defs::{asdf, Thing};\nfn use_it() { let y = asdf(); let _ = y;\n{ let y = 2; let _ = y; } let t = Thing; t.method(); }\n", "= asdf()", "method()", "let y = 2; let _ = y;")
            }
        };
        fs::write(repo.path().join(file), source).unwrap();
        let server_language = if language == "javascript" {
            "typescript"
        } else {
            language
        };
        let mut preferences = app.store.read().unwrap().preferences;
        let mut server = default_servers()
            .into_iter()
            .find(|server| server.id == server_language)
            .unwrap();
        if let Ok(executable) =
            std::env::var(format!("PEER_{}_SERVER", server_language.to_uppercase()))
        {
            server.executable = executable;
        }
        if language == "typescript" {
            if let Ok(directory) = std::env::var("PEER_LSP_LOG_DIR") {
                server.initialization_options["tsserver"]["logDirectory"] =
                    serde_json::json!(directory);
                server.initialization_options["tsserver"]["logVerbosity"] =
                    serde_json::json!("verbose");
            }
        }
        preferences.language_servers = vec![server];
        app.save_preferences(preferences).unwrap();
        let snapshot = app
            .open_repository(
                repo.path().to_str().unwrap(),
                Some(Comparison {
                    view: "code".into(),
                    ..Comparison::default()
                }),
            )
            .await
            .unwrap();
        let position = |marker: &str, symbol: &str| -> Position {
            let (line, text) = source
                .lines()
                .enumerate()
                .find(|(_, line)| line.contains(marker))
                .unwrap();
            let start = text.find(marker).unwrap() + marker.rfind(symbol).unwrap();
            Position {
                line: line as u32,
                character: text[..start].encode_utf16().count() as u32,
            }
        };
        let call = position(marker, "asdf");
        let mut result = None;
        let mut last_error = String::new();
        for _ in 0..50 {
            match app
                .navigate_symbol(request(&snapshot, "new", file, call.line, call.character))
                .await
            {
                Ok(value) if !value.locations.is_empty() => {
                    result = Some(value);
                    break;
                }
                Ok(_) => {}
                Err(error) if error.code == "serverError" => {
                    last_error = error.message;
                }
                Err(error) => panic!("{language}: {error:?}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let result = result
            .unwrap_or_else(|| panic!("{language}: no definition after indexing: {last_error}"));
        assert_eq!(
            result.locations[0].path, definition,
            "{language}: {result:?}"
        );
        let mut reference_request = request(&snapshot, "new", definition, 0, 0);
        reference_request.position = result.locations[0].range.start;
        let references = app.navigate_symbol(reference_request).await.unwrap();
        assert_eq!(references.kind, "references", "{language}");
        assert!(
            references
                .locations
                .iter()
                .any(|location| location.path == file),
            "{language}: {references:?}"
        );
        let method = position(method_marker, "method");
        let result = app
            .navigate_symbol(request(
                &snapshot,
                "new",
                file,
                method.line,
                method.character,
            ))
            .await
            .unwrap();
        assert_eq!(result.locations[0].path, definition, "{language}: method");
        let shadow = position(shadow_marker, "y");
        let result = app
            .navigate_symbol(request(
                &snapshot,
                "new",
                file,
                shadow.line,
                shadow.character,
            ))
            .await
            .unwrap();
        assert_eq!(result.locations[0].path, file, "{language}: local variable");
        assert_eq!(
            result.locations[0].range.start.line, shadow.line,
            "{language}: shadowed declaration"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn package_links_use_snapshot_source_and_refresh_releases_workspaces() {
    let (repo, store, app) = fixture();
    fs::create_dir_all(repo.path().join("packages/lib")).unwrap();
    fs::write(
        repo.path().join("packages/lib/source.ts"),
        "export const version = 'old';\n",
    )
    .unwrap();
    fs::write(repo.path().join("package.json"), "{}").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "package"]);
    fs::write(
        repo.path().join("packages/lib/source.ts"),
        "export const version = 'working';\n",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join("node_modules/dependency")).unwrap();
    fs::write(
        repo.path().join("node_modules/dependency/index.d.ts"),
        "export const installed: number;\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        repo.path().join("packages/lib"),
        repo.path().join("node_modules/local"),
    )
    .unwrap();
    let snapshot = app
        .open_repository(repo.path().to_str().unwrap(), Some(Comparison::default()))
        .await
        .unwrap();
    app.navigate_symbol(request(&snapshot, "old", "call.ts", 0, 1))
        .await
        .unwrap();
    let log = fs::read_to_string(store.path().join("servers.log")).unwrap();
    let root = Path::new(log.lines().last().unwrap());
    assert_eq!(
        fs::read_to_string(root.join("node_modules/local/source.ts")).unwrap(),
        "export const version = 'old';\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("node_modules/dependency/index.d.ts")).unwrap(),
        "export const installed: number;\n"
    );
    assert!(fs::canonicalize(root.join("node_modules/local"))
        .unwrap()
        .starts_with(root));
    let mut preferences = app.store.read().unwrap().preferences;
    preferences.language_servers[0].env.insert(
        "PEER_FAKE_EXTERNAL".into(),
        root.join("node_modules/local/source.ts")
            .to_string_lossy()
            .into_owned(),
    );
    app.save_preferences(preferences.clone()).unwrap();
    let aliased = app
        .navigate_symbol(request(&snapshot, "old", "call.ts", 2, 1))
        .await
        .unwrap();
    assert!(!aliased.locations[0].external);
    assert_eq!(aliased.locations[0].path, "packages/lib/source.ts");
    assert!(aliased.locations[0].label.starts_with("A ·"));
    assert!(source(&app, &aliased).await.contains("'old'"));
    preferences.language_servers[0].env.insert(
        "PEER_FAKE_EXTERNAL".into(),
        repo.path()
            .join("node_modules/local/source.ts")
            .to_string_lossy()
            .into_owned(),
    );
    app.save_preferences(preferences).unwrap();
    assert_eq!(
        app.navigate_symbol(request(&snapshot, "old", "call.ts", 2, 1))
            .await
            .unwrap_err()
            .code,
        "missingSource"
    );
    app.open_repository(repo.path().to_str().unwrap(), Some(Comparison::default()))
        .await
        .unwrap();
    assert!(
        !root.exists(),
        "refresh must release the old temporary source workspace"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("packages/lib/source.ts")).unwrap(),
        "export const version = 'working';\n"
    );
}

#[tokio::test]
async fn large_and_binary_dependency_previews_and_missing_results_are_explicit() {
    let (repo, store, app) = fixture();
    let dependency = store.path().join("dependency.ts");
    fs::write(&dependency, "x".repeat(1024 * 1024 + 1)).unwrap();
    let mut preferences = app.store.read().unwrap().preferences;
    preferences.language_servers[0].env.insert(
        "PEER_FAKE_EXTERNAL".into(),
        dependency.to_str().unwrap().into(),
    );
    app.save_preferences(preferences).unwrap();
    let snapshot = app
        .open_repository(repo.path().to_str().unwrap(), Some(Comparison::default()))
        .await
        .unwrap();
    let result = app
        .navigate_symbol(request(&snapshot, "old", "call.ts", 2, 1))
        .await
        .unwrap();
    let id = &result.locations[0].document_id;
    assert!(
        app.read_navigation_file(&snapshot.id, "old", id, false)
            .await
            .unwrap()
            .deferred
    );
    assert_eq!(
        app.read_navigation_file(&snapshot.id, "old", id, true)
            .await
            .unwrap()
            .content
            .unwrap()
            .len(),
        1024 * 1024 + 1
    );
    fs::write(dependency, [0, 1, 2]).unwrap();
    let snapshot = app
        .open_repository(repo.path().to_str().unwrap(), Some(Comparison::default()))
        .await
        .unwrap();
    let result = app
        .navigate_symbol(request(&snapshot, "old", "call.ts", 2, 1))
        .await
        .unwrap();
    assert_eq!(
        app.read_navigation_file(&snapshot.id, "old", &result.locations[0].document_id, true)
            .await
            .unwrap()
            .kind,
        "binary"
    );
    let result = app
        .navigate_symbol(request(&snapshot, "old", "call.ts", 0, 6))
        .await
        .unwrap();
    assert!(result.locations.is_empty());
}

#[tokio::test]
async fn source_inventory_includes_hidden_untracked_files_and_pins_deleted_files() {
    let (repo, _store, app) = fixture();
    fs::write(repo.path().join("fresh.ts"), "export function fresh() {}\n").unwrap();
    fs::write(repo.path().join("call.ts"), "asdf();\nkeep();\nfresh();\n").unwrap();
    fs::remove_file(repo.path().join("keep.ts")).unwrap();
    let snapshot = app
        .open_repository(
            repo.path().to_str().unwrap(),
            Some(Comparison {
                include_untracked: false,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    assert!(!snapshot.files.iter().any(|file| file.path == "fresh.ts"));
    let result = app
        .navigate_symbol(request(&snapshot, "new", "call.ts", 2, 1))
        .await
        .unwrap();
    assert_eq!(result.locations[0].path, "fresh.ts");
    let deleted = app
        .navigate_symbol(request(&snapshot, "old", "call.ts", 1, 1))
        .await
        .unwrap();
    assert!(source(&app, &deleted).await.contains("export const keep"));
    let missing = app
        .navigate_symbol(request(&snapshot, "new", "call.ts", 1, 1))
        .await
        .unwrap();
    assert!(missing.locations.is_empty());
    app.open_repository(
        repo.path().to_str().unwrap(),
        Some(Comparison {
            mode: "staged".into(),
            ..Default::default()
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        app.navigate_symbol(request(&snapshot, "old", "call.ts", 1, 1))
            .await
            .unwrap_err()
            .code,
        "stale"
    );
    assert_eq!(
        app.read_navigation_file(&snapshot.id, "old", &deleted.locations[0].document_id, true)
            .await
            .unwrap_err()
            .code,
        "stale"
    );
}
