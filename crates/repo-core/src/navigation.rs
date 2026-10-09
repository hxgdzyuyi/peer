use crate::{
    hash,
    lsp::{self, Client},
    repository::{self, RepositorySnapshot},
    types::Comparison,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs,
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as SyncMutex,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{Mutex, Notify},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageServer {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// File extension (without dot) -> LSP language ID.
    pub languages: BTreeMap<String, String>,
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub root_markers: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub initialization_options: Value,
    #[serde(default)]
    pub settings: Value,
}
pub fn default_servers() -> Vec<LanguageServer> {
    serde_json::from_value(json!([
        {"id":"go","name":"Go (gopls)","enabled":true,"languages":{"go":"go"},"executable":"gopls","args":[],"rootMarkers":["go.work","go.mod"],"env":{"GOPROXY":"off","GOTOOLCHAIN":"local","GOFLAGS":"-mod=readonly"},"initializationOptions":{},"settings":{"gopls":{"staticcheck":false}}},
        {"id":"typescript","name":"TypeScript / JavaScript","enabled":true,"languages":{"ts":"typescript","tsx":"typescriptreact","mts":"typescript","cts":"typescript","js":"javascript","jsx":"javascriptreact","mjs":"javascript","cjs":"javascript"},"executable":"typescript-language-server","args":["--stdio"],"rootMarkers":["package.json","tsconfig.json","jsconfig.json"],"env":{},"initializationOptions":{"disableAutomaticTypingAcquisition":true,"tsserver":{"useSyntaxServer":"never"}},"settings":{}},
        {"id":"python","name":"Python (Pyright)","enabled":true,"languages":{"py":"python","pyi":"python"},"executable":"pyright-langserver","args":["--stdio"],"rootMarkers":["pyrightconfig.json","pyproject.toml","setup.py","setup.cfg","requirements.txt","Pipfile"],"env":{},"initializationOptions":{},"settings":{"python":{"analysis":{"autoSearchPaths":true}}}},
        {"id":"rust","name":"Rust (rust-analyzer)","enabled":true,"languages":{"rs":"rust"},"executable":"rust-analyzer","args":[],"rootMarkers":["Cargo.toml","rust-project.json"],"env":{"CARGO_NET_OFFLINE":"true"},"initializationOptions":{"cargo":{"buildScripts":{"enable":false}},"procMacro":{"enable":false},"checkOnSave":false},"settings":{"rust-analyzer":{"cargo":{"buildScripts":{"enable":false}},"procMacro":{"enable":false},"checkOnSave":false}}}
    ])).unwrap()
}
pub fn validate_servers(servers: &[LanguageServer]) -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut extensions = HashSet::new();
    for server in servers {
        if server.id.trim().is_empty()
            || !ids.insert(&server.id)
            || server.executable.trim().is_empty()
            || server.languages.is_empty()
            || server
                .languages
                .iter()
                .any(|(ext, id)| ext.is_empty() || ext.contains(['/', '\\', '.']) || id.is_empty())
            || server.root_markers.iter().any(|marker| {
                marker.is_empty()
                    || Path::new(marker)
                        .components()
                        .any(|c| !matches!(c, Component::Normal(_)))
            })
        {
            return Err("Invalid language-server configuration. Use unique IDs, an executable, extension/language mappings, and relative root markers.".into());
        }
        if server.enabled
            && server
                .languages
                .keys()
                .any(|ext| !extensions.insert(ext.to_lowercase()))
        {
            return Err("Only one enabled language server may handle each file extension".into());
        }
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationError {
    pub code: String,
    pub message: String,
}
impl NavigationError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
    pub(crate) fn io(error: impl std::fmt::Display) -> Self {
        Self::new("missingSource", error.to_string())
    }
}
impl From<String> for NavigationError {
    fn from(message: String) -> Self {
        Self::new(
            if message == repository::STALE || message.starts_with("Snapshot expired") {
                "stale"
            } else {
                "missingSource"
            },
            message,
        )
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRange {
    pub start: Position,
    pub end: Position,
}
impl SourceRange {
    fn contains(&self, position: Position) -> bool {
        self.start <= position && position < self.end
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationRequest {
    pub request_id: String,
    pub snapshot_id: String,
    pub side: String,
    /// Repository-relative path for an initial query, or an issued document ID.
    pub path: Option<String>,
    pub document_id: Option<String>,
    pub position: Position,
    pub operation: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationLocation {
    pub document_id: String,
    pub path: String,
    pub range: SourceRange,
    pub preview: String,
    pub label: String,
    pub external: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationResult {
    pub request_id: String,
    pub snapshot_id: String,
    pub side: String,
    pub kind: String,
    pub locations: Vec<NavigationLocation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationFile {
    pub snapshot_id: String,
    pub side: String,
    pub document_id: String,
    pub path: String,
    pub label: String,
    pub external: bool,
    pub content: Option<String>,
    pub fingerprint: String,
    pub size: u64,
    pub deferred: bool,
    pub kind: String,
    pub tab_width: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct InventoryFile {
    pub blob: Option<String>,
    pub stamp: Option<String>,
    pub symlink: bool,
}
pub type Inventory = BTreeMap<String, InventoryFile>;
pub fn capture_inventories(
    root: &Path,
    comparison: &Comparison,
    all: &BTreeSet<String>,
    base: &BTreeMap<String, String>,
    index: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Inventory>, String> {
    let git_inventory = |metadata: &BTreeMap<String, String>| -> Inventory {
        metadata
            .iter()
            .filter_map(|(path, value)| {
                let fields: Vec<_> = value.split_whitespace().collect();
                let mode = fields.first()?;
                if *mode == "160000" {
                    return None;
                }
                // ls-tree has a type field; ls-files has a stage field.
                let oid = if fields.get(1) == Some(&"blob") {
                    fields.get(2)?
                } else {
                    if fields.get(2) != Some(&"0") {
                        return None;
                    }
                    fields.get(1)?
                };
                Some((
                    path.clone(),
                    InventoryFile {
                        blob: Some((*oid).into()),
                        stamp: None,
                        symlink: *mode == "120000",
                    },
                ))
            })
            .collect()
    };
    let mut working = Inventory::new();
    if comparison.view == "code" || !["staged", "commit"].contains(&comparison.mode.as_str()) {
        for path in all {
            let absolute = repository::safe_path(root, path)?;
            if let Ok(metadata) = fs::symlink_metadata(&absolute) {
                if metadata.is_file() || metadata.file_type().is_symlink() {
                    working.insert(
                        path.clone(),
                        InventoryFile {
                            blob: None,
                            stamp: Some(repository::stamp(root, path)?),
                            symlink: metadata.file_type().is_symlink(),
                        },
                    );
                }
            }
        }
    }
    let old = if comparison.view == "code" {
        Inventory::new()
    } else if comparison.mode == "unstaged" {
        git_inventory(index)
    } else {
        git_inventory(base)
    };
    let new =
        if comparison.view != "code" && ["staged", "commit"].contains(&comparison.mode.as_str()) {
            git_inventory(index)
        } else {
            working
        };
    Ok(BTreeMap::from([("old".into(), old), ("new".into(), new)]))
}
fn version_label(snapshot: &RepositorySnapshot, side: &str) -> String {
    if snapshot.data.comparison.view == "code" {
        return "Working files".into();
    }
    let version = match (snapshot.data.comparison.mode.as_str(), side) {
        ("commit", "new") => snapshot
            .data
            .commit
            .as_ref()
            .map(|c| c.short_id.clone())
            .unwrap_or_default(),
        ("unstaged", "old") | ("staged", "new") => "Index".into(),
        (_, "new") => "Working files".into(),
        _ => snapshot
            .data
            .base_commit
            .as_deref()
            .map(|oid| oid[..oid.len().min(8)].to_string())
            .unwrap_or("Empty tree".into()),
    };
    format!("{} · {version}", if side == "old" { "A" } else { "B" })
}
struct Workspace {
    _directory: tempfile::TempDir,
    root: PathBuf,
    snapshot: Arc<RepositorySnapshot>,
    side: String,
    label: String,
    documents: SyncMutex<HashMap<String, Document>>,
}
#[derive(Clone)]
struct Document {
    absolute: PathBuf,
    path: String,
    external: bool,
    label: String,
    frozen: Option<PathBuf>,
    analysis_root: Option<PathBuf>,
}
impl Workspace {
    fn issue(
        &self,
        absolute: &Path,
        analysis_root: Option<&Path>,
    ) -> Result<(String, Document), NavigationError> {
        let root = &self.root;
        // A source in the real repository must never masquerade as an external dependency.
        let original = Path::new(&self.snapshot.data.repository.root);
        let mut resolved = normalized(absolute);
        if let Ok(relative) = absolute.strip_prefix(original) {
            let relative = relative.to_string_lossy().replace('\\', "/");
            if !relative.split('/').any(|part| part == "node_modules") {
                return Err(NavigationError::new(
                    "missingSource",
                    "The language server resolved live repository source outside the selected snapshot. Use project-relative imports and root configuration.",
                ));
            }
        }
        let mut relative = resolved
            .strip_prefix(root)
            .ok()
            .map(|path| path.to_string_lossy().replace('\\', "/"));
        let mut internal = relative
            .as_ref()
            .is_some_and(|path| self.snapshot.navigation[&self.side].contains_key(path));
        let canonical = fs::canonicalize(&resolved).map_err(NavigationError::io)?;
        if canonical.strip_prefix(original).is_ok_and(|path| {
            !path
                .components()
                .any(|part| part.as_os_str() == "node_modules")
        }) {
            return Err(NavigationError::new("missingSource", "The language server resolved live repository source outside the selected snapshot. Use project-relative imports and root configuration."));
        }
        if !internal {
            if let Ok(path) = canonical.strip_prefix(root) {
                let path = path.to_string_lossy().replace('\\', "/");
                if self.snapshot.navigation[&self.side].contains_key(&path) {
                    // A package-manager alias of repository source still
                    // belongs to A/B, not to an unversioned dependency.
                    relative = Some(path);
                    resolved = canonical.clone();
                    internal = true;
                }
            }
        }
        let analysis_root = if internal { None } else { analysis_root };
        // The workspace only creates internal source links; reject any unexpected escape.
        if internal && !canonical.starts_with(root) {
            return Err(NavigationError::new(
                "missingSource",
                "Source link escapes the selected snapshot",
            ));
        }
        let path = if internal {
            relative.unwrap()
        } else {
            resolved.to_string_lossy().to_string()
        };
        let id = hash(
            format!(
                "{}:{}:{}:{}",
                self.snapshot.data.id,
                self.side,
                resolved.display(),
                analysis_root.unwrap_or(root).display()
            )
            .as_bytes(),
        );
        let mut documents = self.documents.lock().unwrap();
        if let Some(document) = documents.get(&id) {
            return Ok((id, document.clone()));
        }
        let mut document = Document {
            absolute: resolved,
            path,
            external: !internal,
            label: if internal {
                self.label.clone()
            } else {
                if is_standard_library(&canonical) {
                    "Standard library".into()
                } else {
                    "Installed dependency".into()
                }
            },
            frozen: None,
            analysis_root: analysis_root.map(Path::to_path_buf),
        };
        if !internal {
            let size = fs::metadata(&canonical).map_err(NavigationError::io)?.len();
            if size > 64 * 1024 * 1024 {
                return Err(NavigationError::new(
                    "missingSource",
                    "Dependency source exceeds the 64 MiB inspection limit",
                ));
            }
            let cache = self._directory.path().join("external");
            fs::create_dir_all(&cache).map_err(NavigationError::io)?;
            let frozen = cache.join(&id);
            fs::copy(&canonical, &frozen).map_err(NavigationError::io)?;
            document.frozen = Some(frozen);
        }
        documents.insert(id.clone(), document.clone());
        Ok((id, document))
    }
    fn document(&self, id: &str) -> Result<Document, NavigationError> {
        self.documents
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| NavigationError::new("missingSource", "Unknown navigation document"))
    }
    fn read(&self, id: &str, force: bool) -> Result<NavigationFile, NavigationError> {
        let document = self.document(id)?;
        let source = document.frozen.as_ref().unwrap_or(&document.absolute);
        let size = fs::metadata(source).map_err(NavigationError::io)?.len();
        if size > 64 * 1024 * 1024 {
            return Err(NavigationError::new(
                "missingSource",
                "Source exceeds the 64 MiB inspection limit",
            ));
        }
        let tab_width = if document.external {
            None
        } else {
            repository::editorconfig_tab_width(&self.root, &document.path)
        };
        let mut result = NavigationFile {
            snapshot_id: self.snapshot.data.id.clone(),
            side: self.side.clone(),
            document_id: id.into(),
            path: document.path,
            label: document.label,
            external: document.external,
            content: None,
            fingerprint: String::new(),
            size,
            deferred: size > repository::MAX_BYTES && !force,
            kind: "text".into(),
            tab_width,
        };
        if result.deferred {
            return Ok(result);
        }
        let bytes = fs::read(source).map_err(NavigationError::io)?;
        result.fingerprint = hash(&bytes);
        if bytes.contains(&0) {
            result.kind = "binary".into();
        } else if let Ok(content) = String::from_utf8(bytes) {
            if !force && content.lines().count() as u64 > repository::MAX_LINES {
                result.deferred = true;
            } else {
                result.content = Some(content);
            }
        } else {
            result.kind = "binary".into();
        }
        Ok(result)
    }
}

fn is_standard_library(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized.contains("/lib/rustlib/")
        || normalized.contains("/typescript/lib/lib.") && normalized.ends_with(".d.ts")
    {
        return true;
    }
    // Go toolchains may be under gvm, Homebrew, other version managers,
    // or the module cache. Recognize their standard-library module itself.
    path.ancestors()
        .filter(|ancestor| ancestor.file_name().is_some_and(|name| name == "src"))
        .any(|source| {
            fs::read_to_string(source.join("go.mod"))
                .is_ok_and(|manifest| manifest.lines().any(|line| line.trim() == "module std"))
        })
}

fn normalized(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            _ => result.push(component.as_os_str()),
        }
    }
    result
}
fn source_link(target: &Path, destination: &Path) -> Result<(), NavigationError> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, destination).map_err(NavigationError::io)
    }
    #[cfg(windows)]
    {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, destination)
        } else {
            std::os::windows::fs::symlink_file(target, destination)
        }
        .map_err(NavigationError::io)
    }
}
// Copy installed dependencies, never hard-link them to the original repository.
// Package-manager links into project source are redirected into the snapshot.
fn copy_dependency<'a>(
    source: &'a Path,
    destination: &'a Path,
    original: &'a Path,
    workspace: &'a Path,
    ancestors: &'a mut HashSet<PathBuf>,
    remaining: &'a mut u64,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigationError>> + Send + 'a>> {
    Box::pin(async move {
        // Let superseding requests cancel preparation between files.
        tokio::task::yield_now().await;
        if destination.exists() {
            return Ok(());
        }
        let canonical = fs::canonicalize(source).map_err(NavigationError::io)?;
        if let Ok(relative) = canonical.strip_prefix(original) {
            if !relative
                .components()
                .any(|c| c.as_os_str() == "node_modules")
            {
                let target = workspace.join(relative);
                if target.exists() {
                    return source_link(&target, destination);
                }
                return Ok(()); // Project package absent at this revision.
            }
        }
        if !ancestors.insert(canonical.clone()) {
            return Ok(());
        }
        let metadata = fs::metadata(&canonical).map_err(NavigationError::io)?;
        if metadata.is_dir() {
            fs::create_dir_all(destination).map_err(NavigationError::io)?;
            for entry in fs::read_dir(&canonical).map_err(NavigationError::io)? {
                let entry = entry.map_err(NavigationError::io)?;
                if entry.file_name() == ".bin" || entry.file_name() == ".cache" {
                    continue;
                }
                copy_dependency(
                    &entry.path(),
                    &destination.join(entry.file_name()),
                    original,
                    workspace,
                    ancestors,
                    remaining,
                )
                .await?;
            }
        } else if metadata.is_file() {
            *remaining = remaining.checked_sub(metadata.len()).ok_or_else(|| {
                NavigationError::new(
                    "missingSource",
                    "Installed dependencies exceed the 2 GiB temporary workspace limit",
                )
            })?;
            fs::copy(&canonical, destination).map_err(NavigationError::io)?;
        }
        ancestors.remove(&canonical);
        Ok(())
    })
}
async fn materialize(
    snapshot: Arc<RepositorySnapshot>,
    side: &str,
) -> Result<Workspace, NavigationError> {
    let inventory = snapshot
        .navigation
        .get(side)
        .ok_or_else(|| NavigationError::new("missingSource", "Choose the old or new side"))?;
    let original = Path::new(&snapshot.data.repository.root);
    let validate = || -> Result<(), NavigationError> {
        for (path, source) in inventory {
            if let Some(stamp) = &source.stamp {
                if repository::stamp(original, path)? != *stamp {
                    return Err(repository::STALE.to_string().into());
                }
            }
        }
        Ok(())
    };
    validate()?;
    if inventory.values().any(|file| file.stamp.is_some())
        && repository::signature(&snapshot.git, original).await? != snapshot.signature
    {
        return Err(repository::STALE.to_string().into());
    }
    let directory = tempfile::Builder::new()
        .prefix("peer-navigation-")
        .tempdir()
        .map_err(NavigationError::io)?;
    // Canonical roots matter on macOS, where /var and /tmp are symlinks.
    let root = fs::canonicalize(directory.path())
        .map_err(NavigationError::io)?
        .join("source");
    fs::create_dir(&root).map_err(NavigationError::io)?;
    let mut child = Command::new(&snapshot.git)
        .args(["cat-file", "--batch"])
        .current_dir(original)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(NavigationError::io)?;
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut links = Vec::new();
    for (path, source) in inventory {
        tokio::task::yield_now().await;
        let destination = repository::safe_path(&root, path)?;
        fs::create_dir_all(destination.parent().unwrap()).map_err(NavigationError::io)?;
        let bytes = if let Some(blob) = &source.blob {
            stdin
                .write_all(format!("{blob}\n").as_bytes())
                .await
                .map_err(NavigationError::io)?;
            stdin.flush().await.map_err(NavigationError::io)?;
            let mut header = String::new();
            stdout
                .read_line(&mut header)
                .await
                .map_err(NavigationError::io)?;
            let size = header
                .split_whitespace()
                .nth(2)
                .and_then(|size| size.parse::<u64>().ok())
                .ok_or_else(|| {
                    NavigationError::new("missingSource", "Git object is unavailable")
                })?;
            if size > 64 * 1024 * 1024 {
                tokio::io::copy(&mut (&mut stdout).take(size + 1), &mut tokio::io::sink())
                    .await
                    .map_err(NavigationError::io)?;
                continue;
            }
            let mut bytes = vec![0; size as usize];
            stdout
                .read_exact(&mut bytes)
                .await
                .map_err(NavigationError::io)?;
            stdout.read_u8().await.map_err(NavigationError::io)?;
            bytes
        } else {
            let source_path = repository::safe_path(original, path)?;
            if source.symlink {
                fs::read_link(&source_path)
                    .map_err(NavigationError::io)?
                    .to_string_lossy()
                    .as_bytes()
                    .to_vec()
            } else {
                if fs::metadata(&source_path)
                    .map_err(NavigationError::io)?
                    .len()
                    > 64 * 1024 * 1024
                {
                    continue;
                }
                fs::read(&source_path).map_err(NavigationError::io)?
            }
        };
        if source.symlink {
            links.push((destination, bytes));
        } else {
            fs::write(destination, bytes).map_err(NavigationError::io)?;
        }
    }
    drop(stdin);
    let _ = child.wait().await;
    validate()?;
    for (destination, bytes) in links {
        let Ok(target) = String::from_utf8(bytes) else {
            continue;
        };
        let target = Path::new(&target);
        let resolved = if target.is_absolute() {
            target
                .strip_prefix(original)
                .ok()
                .map(|relative| root.join(relative))
        } else {
            Some(normalized(&destination.parent().unwrap().join(target)))
        };
        if let Some(target) = resolved.filter(|target| target.starts_with(&root)) {
            source_link(&target, &destination)?;
        }
    }
    let mut dependency_roots = BTreeSet::new();
    for path in inventory.keys().filter(|path| {
        path.ends_with("package.json")
            || path.ends_with("tsconfig.json")
            || path.ends_with("jsconfig.json")
    }) {
        let mut parent = Path::new(path).parent();
        while let Some(relative) = parent {
            dependency_roots.insert(relative.join("node_modules"));
            parent = relative.parent();
        }
    }
    let mut remaining = 2 * 1024 * 1024 * 1024u64;
    for relative in dependency_roots {
        let source = original.join(&relative);
        if source.is_dir() && !root.join(&relative).exists() {
            copy_dependency(
                &source,
                &root.join(&relative),
                original,
                &root,
                &mut HashSet::new(),
                &mut remaining,
            )
            .await?;
        }
    }
    validate()?;
    Ok(Workspace {
        _directory: directory,
        root,
        label: version_label(&snapshot, side),
        snapshot,
        side: side.into(),
        documents: SyncMutex::new(HashMap::new()),
    })
}

struct Session {
    client: Client,
    _workspace: Arc<Workspace>,
    opened: Mutex<VecDeque<String>>,
    used: SyncMutex<Instant>,
}
struct Cancellation {
    cancelled: AtomicBool,
    notify: Notify,
}
impl Cancellation {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_one();
    }
    async fn wait(&self) {
        if !self.cancelled.load(Ordering::SeqCst) {
            self.notify.notified().await;
        }
    }
}
#[derive(Default)]
struct State {
    workspaces: HashMap<String, Arc<Workspace>>,
    sessions: HashMap<String, Arc<Session>>,
}
impl State {
    fn take_idle_sessions(&mut self) -> Vec<Arc<Session>> {
        let expired: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, session)| {
                Arc::strong_count(session) == 1
                    && session.used.lock().unwrap().elapsed() >= Duration::from_secs(300)
            })
            .map(|(key, _)| key.clone())
            .collect();
        expired
            .into_iter()
            .filter_map(|key| self.sessions.remove(&key))
            .collect()
    }
}
#[derive(Default)]
pub struct NavigationService {
    state: Arc<Mutex<State>>,
    requests: SyncMutex<HashMap<String, Arc<Cancellation>>>,
    active_snapshot: SyncMutex<Option<String>>,
    reaper_started: AtomicBool,
}
impl NavigationService {
    pub async fn activate(&self, snapshot_id: &str) {
        *self.active_snapshot.lock().unwrap() = Some(snapshot_id.into());
        self.invalidate().await;
    }
    fn check_active(&self, snapshot_id: &str) -> Result<(), NavigationError> {
        if self
            .active_snapshot
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|active| active != snapshot_id)
        {
            return Err(NavigationError::new(
                "stale",
                "Snapshot expired. Refresh the repository.",
            ));
        }
        Ok(())
    }
    pub fn cancel(&self, id: &str) {
        if let Some(request) = self.requests.lock().unwrap().get(id) {
            request.cancel();
        }
    }
    pub async fn invalidate(&self) {
        for request in self.requests.lock().unwrap().values() {
            request.cancel();
        }
        let mut state = self.state.lock().await;
        let sessions = std::mem::take(&mut state.sessions);
        state.workspaces.clear();
        drop(state);
        for session in sessions.values() {
            session.client.shutdown().await;
        }
    }
    fn reap(&self) {
        if self.reaper_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(&self.state);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let Some(state) = weak.upgrade() else { break };
                let mut state = state.lock().await;
                let retired = state.take_idle_sessions();
                drop(state);
                for session in retired {
                    session.client.shutdown().await;
                }
            }
        });
    }
    async fn workspace(
        &self,
        snapshot: Arc<RepositorySnapshot>,
        side: &str,
    ) -> Result<Arc<Workspace>, NavigationError> {
        if !["old", "new"].contains(&side) {
            return Err(NavigationError::new(
                "missingSource",
                "Invalid navigation side",
            ));
        }
        let key = format!("{}:{side}", snapshot.data.id);
        let mut state = self.state.lock().await;
        if let Some(workspace) = state.workspaces.get(&key) {
            return Ok(workspace.clone());
        }
        let workspace = Arc::new(materialize(snapshot, side).await?);
        state.workspaces.insert(key, workspace.clone());
        Ok(workspace)
    }
    pub async fn read(
        &self,
        snapshot: Arc<RepositorySnapshot>,
        side: &str,
        document_id: &str,
        force: bool,
    ) -> Result<NavigationFile, NavigationError> {
        self.check_active(&snapshot.data.id)?;
        let key = format!("{}:{side}", snapshot.data.id);
        let workspace = self
            .state
            .lock()
            .await
            .workspaces
            .get(&key)
            .cloned()
            .ok_or_else(|| NavigationError::new("missingSource", "Unknown navigation document"))?;
        let result = workspace.read(document_id, force);
        self.check_active(&snapshot.data.id)?;
        result
    }
    pub async fn query(
        &self,
        snapshot: Arc<RepositorySnapshot>,
        servers: &[LanguageServer],
        request: NavigationRequest,
    ) -> Result<NavigationResult, NavigationError> {
        self.reap();
        let cancel = {
            // Register while holding the context lock so invalidation cannot
            // miss a request between its context check and cancellation setup.
            let active = self.active_snapshot.lock().unwrap();
            if active.as_ref().is_some_and(|id| id != &request.snapshot_id) {
                return Err(NavigationError::new(
                    "stale",
                    "Snapshot expired. Refresh the repository.",
                ));
            }
            let cancel = Arc::new(Cancellation {
                cancelled: AtomicBool::new(false),
                notify: Notify::new(),
            });
            if let Some(previous) = self
                .requests
                .lock()
                .unwrap()
                .insert(request.request_id.clone(), cancel.clone())
            {
                previous.cancel();
            }
            cancel
        };
        let result = tokio::select! {
            result = self.query_inner(snapshot, servers, &request) => result,
            _ = cancel.wait() => Err(NavigationError::new("cancelled", "Navigation cancelled")),
        };
        {
            let mut requests = self.requests.lock().unwrap();
            if requests
                .get(&request.request_id)
                .is_some_and(|current| Arc::ptr_eq(current, &cancel))
            {
                requests.remove(&request.request_id);
            }
        }
        self.check_active(&request.snapshot_id)?;
        if cancel.cancelled.load(Ordering::SeqCst) {
            return Err(NavigationError::new("cancelled", "Navigation cancelled"));
        }
        result
    }
    async fn query_inner(
        &self,
        snapshot: Arc<RepositorySnapshot>,
        servers: &[LanguageServer],
        request: &NavigationRequest,
    ) -> Result<NavigationResult, NavigationError> {
        if !["auto", "references"].contains(&request.operation.as_str()) {
            return Err(NavigationError::new(
                "unsupported",
                "Unknown navigation operation",
            ));
        }
        if let Some(path) = &request.path {
            let extension = Path::new(path)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !servers.iter().any(|server| {
                server.enabled
                    && server
                        .languages
                        .keys()
                        .any(|ext| ext.eq_ignore_ascii_case(&extension))
            }) {
                return Err(NavigationError::new("missingServer", format!("No language server configured for .{extension}. Add one in Settings → Code navigation.")));
            }
        }
        let workspace = self.workspace(snapshot.clone(), &request.side).await?;
        let (document_id, document) = if let Some(id) = &request.document_id {
            (id.clone(), workspace.document(id)?)
        } else {
            let path = request
                .path
                .as_deref()
                .ok_or_else(|| NavigationError::new("missingSource", "Select a source position"))?;
            if !workspace.snapshot.navigation[&request.side].contains_key(path) {
                return Err(NavigationError::new(
                    "missingSource",
                    "File is absent from this version",
                ));
            }
            workspace.issue(&repository::safe_path(&workspace.root, path)?, None)?
        };
        let extension = Path::new(&document.path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        let server = servers.iter().find(|server| server.enabled && server.languages.keys().any(|ext| ext.eq_ignore_ascii_case(&extension))).ok_or_else(|| NavigationError::new("missingServer", format!("No language server configured for .{extension}. Add one in Settings → Code navigation.")))?;
        let language = server
            .languages
            .iter()
            .find(|(ext, _)| ext.eq_ignore_ascii_case(&extension))
            .map(|(_, language)| language)
            .unwrap();
        let root = document.analysis_root.clone().unwrap_or_else(|| {
            project_root(&workspace.root, &document.absolute, &server.root_markers)
        });
        let key = hash(
            format!(
                "{}:{}:{}:{}",
                snapshot.data.id,
                request.side,
                root.display(),
                serde_json::to_string(server).unwrap()
            )
            .as_bytes(),
        );
        let session = {
            let mut state = self.state.lock().await;
            if state
                .sessions
                .get(&key)
                .is_some_and(|session| !session.client.alive())
            {
                state.sessions.remove(&key);
            }
            if let Some(session) = state.sessions.get(&key) {
                session.clone()
            } else {
                while state.sessions.len() >= 4 {
                    let oldest = state
                        .sessions
                        .iter()
                        .filter(|(_, session)| Arc::strong_count(session) == 1)
                        .min_by_key(|(_, session)| *session.used.lock().unwrap())
                        .map(|(key, _)| key.clone())
                        .ok_or_else(|| {
                            NavigationError::new(
                                "busy",
                                "Other language servers are busy. Retry shortly.",
                            )
                        })?;
                    if let Some(session) = state.sessions.remove(&oldest) {
                        session.client.shutdown().await;
                    }
                }
                let session = Arc::new(Session {
                    client: Client::start(server, &root).await?,
                    _workspace: workspace.clone(),
                    opened: Mutex::new(VecDeque::new()),
                    used: SyncMutex::new(Instant::now()),
                });
                state.sessions.insert(key, session.clone());
                session
            }
        };
        *session.used.lock().unwrap() = Instant::now();
        let file = workspace.read(&document_id, true)?;
        let text = file.content.ok_or_else(|| {
            NavigationError::new("missingSource", "Symbol navigation requires a text file")
        })?;
        let line = text
            .split('\n')
            .nth(request.position.line as usize)
            .ok_or_else(|| {
                NavigationError::new("missingSource", "Source position is outside the file")
            })?;
        if request.position.character as usize > line.encode_utf16().count() {
            return Err(NavigationError::new(
                "missingSource",
                "Source position is outside the line",
            ));
        }
        let uri = lsp::uri(&document.absolute)?;
        let mut opened = session.opened.lock().await;
        if let Some(index) = opened.iter().position(|document| *document == uri) {
            opened.remove(index);
        } else {
            session.client.notify(
                "textDocument/didOpen",
                json!({"textDocument":{"uri":uri,"languageId":language,"version":1,"text":text}}),
            )?;
        }
        opened.push_back(uri.clone());
        while opened.len() > 32 {
            if let Some(uri) = opened.pop_front() {
                session
                    .client
                    .notify("textDocument/didClose", json!({"textDocument":{"uri":uri}}))?;
            }
        }
        drop(opened);
        let params = json!({"textDocument":{"uri":uri},"position":request.position});
        let mut references = request.operation == "references";
        let mut locations = vec![];
        if !references {
            ensure_capability(&session.client, "definitionProvider")?;
            let response = session
                .client
                .request(
                    "textDocument/definition",
                    params.clone(),
                    Duration::from_secs(30),
                )
                .await?;
            locations = normalize_locations(&response)?;
            let canonical = fs::canonicalize(&document.absolute).ok();
            references = locations.iter().any(|(target, range)| {
                range.contains(request.position)
                    && (*target == uri
                        || canonical.is_some()
                            && lsp::file_path(target)
                                .ok()
                                .and_then(|path| fs::canonicalize(path).ok())
                                == canonical)
            });
        }
        if references {
            ensure_capability(&session.client, "referencesProvider")?;
            let response = session.client.request("textDocument/references", json!({"textDocument":{"uri":uri},"position":request.position,"context":{"includeDeclaration":false}}), Duration::from_secs(30)).await?;
            locations = normalize_locations(&response)?;
        }
        let mut results = Vec::new();
        let mut seen = HashSet::new();
        for (uri, range) in locations {
            if !seen.insert(format!("{uri}:{range:?}")) {
                continue;
            }
            let path = lsp::file_path(&uri)?;
            let (id, document) = workspace.issue(&path, Some(&root))?;
            let file = workspace.read(&id, false)?;
            let preview = file
                .content
                .as_deref()
                .and_then(|text| text.split('\n').nth(range.start.line as usize))
                .unwrap_or("")
                .trim()
                .chars()
                .take(300)
                .collect();
            results.push(NavigationLocation {
                document_id: id,
                path: document.path,
                range,
                preview,
                label: document.label,
                external: document.external,
            });
        }
        results.sort_by(|a, b| (&a.path, a.range.start).cmp(&(&b.path, b.range.start)));
        *session.used.lock().unwrap() = Instant::now();
        Ok(NavigationResult {
            request_id: request.request_id.clone(),
            snapshot_id: snapshot.data.id.clone(),
            side: request.side.clone(),
            kind: if references {
                "references"
            } else {
                "definitions"
            }
            .into(),
            locations: results,
        })
    }
}
fn project_root(workspace: &Path, document: &Path, markers: &[String]) -> PathBuf {
    for marker in markers {
        let mut current = document.parent();
        while let Some(directory) = current.filter(|directory| directory.starts_with(workspace)) {
            if directory.join(marker).is_file() {
                return directory.to_path_buf();
            }
            current = directory.parent();
        }
    }
    workspace.to_path_buf()
}
fn ensure_capability(client: &Client, name: &str) -> Result<(), NavigationError> {
    match client.capabilities.get(name) {
        Some(Value::Bool(true)) | Some(Value::Object(_)) => Ok(()),
        _ => Err(NavigationError::new(
            "unsupported",
            format!(
                "The language server does not support {}",
                if name == "referencesProvider" {
                    "references"
                } else {
                    "definitions"
                }
            ),
        )),
    }
}
fn normalize_locations(value: &Value) -> Result<Vec<(String, SourceRange)>, NavigationError> {
    let values = match value {
        Value::Null => vec![],
        Value::Array(values) => values.iter().collect(),
        _ => vec![value],
    };
    values
        .into_iter()
        .map(|value| {
            let uri = value
                .get("targetUri")
                .or_else(|| value.get("uri"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    NavigationError::new("serverError", "Invalid definition location")
                })?;
            let range = value
                .get("targetSelectionRange")
                .or_else(|| value.get("range"))
                .ok_or_else(|| NavigationError::new("serverError", "Invalid definition range"))?;
            let range: SourceRange = serde_json::from_value(range.clone())
                .map_err(|error| NavigationError::new("serverError", error.to_string()))?;
            if range.end < range.start {
                return Err(NavigationError::new("serverError", "Reversed source range"));
            }
            Ok((uri.into(), range))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn working_copy_races_reject_the_workspace() {
        use std::os::unix::fs::PermissionsExt;
        let repository = tempfile::tempdir().unwrap();
        let control = tempfile::tempdir().unwrap();
        assert!(std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repository.path())
            .status()
            .unwrap()
            .success());
        let source = repository.path().join("source.ts");
        fs::write(&source, "export const value = 1;\n").unwrap();
        let mut snapshot = repository::create_snapshot(
            repository.path().to_str().unwrap(),
            Comparison {
                view: "code".into(),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
        let marker = control.path().join("started");
        let ready = control.path().join("ready");
        let wrapper = control.path().join("git-fixture");
        let quote = |text: &str| format!("'{}'", text.replace('\'', "'\"'\"'"));
        fs::write(&wrapper, format!("#!/bin/sh\nif [ \"$1\" = cat-file ]; then\n touch {}\n while [ ! -f {} ]; do sleep 0.01; done\nfi\nexec {} \"$@\"\n", quote(marker.to_str().unwrap()), quote(ready.to_str().unwrap()), quote(&snapshot.git))).unwrap();
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
        snapshot.git = wrapper.to_string_lossy().into_owned();
        let task = tokio::spawn(async move { materialize(Arc::new(snapshot), "new").await });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !marker.exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        fs::write(&source, "export const value = 2;\n").unwrap();
        fs::write(&ready, "").unwrap();
        match task.await.unwrap() {
            Err(error) => assert_eq!(error.code, "stale"),
            Ok(_) => panic!("A racing workspace must never be published"),
        }
    }

    #[tokio::test]
    async fn sessions_documents_and_idle_retirement_are_bounded() {
        let repository = tempfile::tempdir().unwrap();
        assert!(std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repository.path())
            .status()
            .unwrap()
            .success());
        for index in 0..34 {
            fs::write(
                repository.path().join(format!("file{index}.ts")),
                "target();\n",
            )
            .unwrap();
        }
        fs::write(
            repository.path().join("defs.ts"),
            "export function target() {}\n",
        )
        .unwrap();
        let snapshot = Arc::new(
            repository::create_snapshot(
                repository.path().to_str().unwrap(),
                Comparison {
                    view: "code".into(),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap(),
        );
        let service = NavigationService::default();
        let mut server = default_servers().remove(1);
        server.executable = "node".into();
        server.args = vec![Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/fake-lsp.mjs")
            .to_string_lossy()
            .into_owned()];
        let request = |index: usize| NavigationRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            snapshot_id: snapshot.data.id.clone(),
            side: "new".into(),
            path: Some(format!("file{index}.ts")),
            document_id: None,
            position: Position {
                line: 0,
                character: 1,
            },
            operation: "auto".into(),
        };
        let mut first = None;
        for index in 0..5 {
            server.id = format!("server{index}");
            service
                .query(snapshot.clone(), &[server.clone()], request(0))
                .await
                .unwrap();
            let state = service.state.lock().await;
            if index == 0 {
                first = Some(state.sessions.values().next().unwrap().client.clone());
            }
            assert!(state.sessions.len() <= 4);
        }
        assert!(!first.unwrap().alive());
        for index in 1..34 {
            service
                .query(snapshot.clone(), &[server.clone()], request(index))
                .await
                .unwrap();
        }
        let mut state = service.state.lock().await;
        let mut maximum = 0;
        for session in state.sessions.values() {
            maximum = maximum.max(session.opened.lock().await.len());
            *session.used.lock().unwrap() = Instant::now() - Duration::from_secs(301);
        }
        assert_eq!(maximum, 32);
        let roots: Vec<_> = state
            .workspaces
            .values()
            .map(|workspace| workspace.root.clone())
            .collect();
        let retired = state.take_idle_sessions();
        assert_eq!(retired.len(), 4);
        assert!(state.sessions.is_empty());
        drop(state);
        for session in &retired {
            session.client.shutdown().await;
        }
        drop(retired);
        service.invalidate().await;
        assert!(roots.iter().all(|path| !path.exists()));
    }
}
