#[path = "file_tree.rs"]
mod file_tree;
#[path = "settings.rs"]
mod settings;
use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        diff::*,
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        *,
    },
    *,
};
use peer_core::Inspector;
use peer_core::{
    navigation::{NavigationFile, NavigationRequest, NavigationResult, Position},
    presentation, *,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

actions!(
    peer,
    [
        Open,
        Refresh,
        QuickOpen,
        Find,
        GoLine,
        Back,
        Settings,
        Definition,
        References,
        Cancel,
        Quit,
        NextFile,
        PreviousFile,
        TreeUp,
        TreeDown,
        TreeLeft,
        TreeRight,
        TreeConfirm,
        TreeFirst,
        TreeLast,
        SaveComment
    ]
);
pub fn init(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.set_menus([
        Menu::new("Peer").items([
            MenuItem::action("Settings…", Settings),
            MenuItem::separator(),
            MenuItem::action("Quit Peer", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Open repository…", Open),
            MenuItem::action("Refresh", Refresh),
        ]),
        Menu::new("Edit").items([
            MenuItem::action("Undo", input::Undo),
            MenuItem::action("Redo", input::Redo),
            MenuItem::separator(),
            MenuItem::action("Cut", input::Cut),
            MenuItem::action("Copy", input::Copy),
            MenuItem::action("Paste", input::Paste),
            MenuItem::action("Select all", input::SelectAll),
        ]),
        Menu::new("Navigate").items([
            MenuItem::action("Go to file…", QuickOpen),
            MenuItem::action("Find", Find),
            MenuItem::action("Go to line", GoLine),
            MenuItem::action("Back", Back),
            MenuItem::action("Definition", Definition),
            MenuItem::action("References", References),
        ]),
    ]);
    cx.bind_keys([
        KeyBinding::new("up", TreeUp, Some("PeerFileTree")),
        KeyBinding::new("down", TreeDown, Some("PeerFileTree")),
        KeyBinding::new("left", TreeLeft, Some("PeerFileTree")),
        KeyBinding::new("right", TreeRight, Some("PeerFileTree")),
        KeyBinding::new("enter", TreeConfirm, Some("PeerFileTree")),
        KeyBinding::new("space", TreeConfirm, Some("PeerFileTree")),
        KeyBinding::new("home", TreeFirst, Some("PeerFileTree")),
        KeyBinding::new("end", TreeLast, Some("PeerFileTree")),
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("down", NextFile, Some("PeerQuickOpen")),
        KeyBinding::new("down", NextFile, Some("PeerFileFilter")),
        KeyBinding::new("up", PreviousFile, Some("PeerFileFilter")),
        KeyBinding::new("up", PreviousFile, Some("PeerQuickOpen")),
        KeyBinding::new("ctrl-n", NextFile, Some("PeerQuickOpen")),
        KeyBinding::new("ctrl-p", PreviousFile, Some("PeerQuickOpen")),
        KeyBinding::new("secondary-o", Open, Some("Peer")),
        KeyBinding::new("secondary-r", Refresh, Some("Peer")),
        KeyBinding::new("secondary-p", QuickOpen, Some("Peer")),
        KeyBinding::new("secondary-f", Find, Some("Peer")),
        KeyBinding::new("secondary-g", GoLine, Some("Peer")),
        KeyBinding::new("alt-left", Back, Some("Peer")),
        KeyBinding::new("ctrl--", Back, Some("Peer")),
        KeyBinding::new("secondary-,", Settings, Some("Peer")),
        KeyBinding::new("f12", Definition, Some("Peer")),
        KeyBinding::new("shift-f12", References, Some("Peer")),
        KeyBinding::new("escape", Cancel, Some("Peer")),
        KeyBinding::new("secondary-enter", SaveComment, Some("Peer")),
    ]);
}

#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct SavedView {
    body: String,
    fingerprint: Option<String>,
    pending_new: Option<Box<SavedView>>,
    side: Option<String>,
    line: Option<usize>,
    editing: Option<String>,
    scroll_line: Option<usize>,
    scroll_side: Option<String>,
    scroll_offset: f32,
    horizontal: f32,
}

impl SavedView {
    fn after_submission(&self, submitted: &Self, restored: &Self) -> Option<Self> {
        if self.same_draft(submitted) {
            return Some(restored.clone());
        }
        if submitted.editing.is_none()
            && self
                .pending_new
                .as_deref()
                .is_some_and(|v| v.same_draft(submitted))
        {
            let mut next = self.clone();
            next.pending_new = Some(Box::new(restored.clone()));
            return Some(next);
        }
        if submitted.editing.is_some()
            && self.editing == submitted.editing
            && self.body == submitted.body
        {
            return Some(self.pending_new.as_deref().cloned().unwrap_or_default());
        }
        None
    }
    fn same_draft(&self, other: &Self) -> bool {
        self.body == other.body
            && self.side == other.side
            && self.line == other.line
            && self.editing == other.editing
            && self.fingerprint == other.fingerprint
            && self.pending_new == other.pending_new
    }
}

fn legacy_draft(
    views: &BTreeMap<String, SavedView>,
    key: &str,
    fingerprint: &str,
) -> Option<SavedView> {
    let exact = views.get(&format!("{key}:{fingerprint}"));
    exact
        .filter(|v| !v.body.is_empty() || v.pending_new.is_some())
        .map(|v| (fingerprint.to_owned(), v.clone()))
        .or_else(|| {
            views.iter().find_map(|(k, v)| {
                k.strip_prefix(&format!("{key}:"))
                    .filter(|fp| {
                        fp.len() == 64
                            && fp.bytes().all(|b| b.is_ascii_hexdigit())
                            && (!v.body.is_empty() || v.pending_new.is_some())
                    })
                    .map(|fp| (fp.to_string(), v.clone()))
            })
        })
        .or_else(|| exact.map(|v| (fingerprint.to_owned(), v.clone())))
        .map(|(fp, mut v)| {
            v.fingerprint.get_or_insert(fp);
            v
        })
}

pub struct Workspace {
    pending_jobs: std::cell::RefCell<Vec<tokio::task::JoinHandle<()>>>,
    core: Arc<Inspector>,
    runtime: Arc<tokio::runtime::Runtime>,
    gate: Arc<tokio::sync::Mutex<()>>,
    epoch: Arc<AtomicU64>,
    file_epoch: u64,
    themes: Vec<ImportedTheme>,
    feedback_pending: bool,
    feedback_visible: bool,
    clear_queue_request: Option<String>,
    pending_jump: Option<DiffLinePosition>,
    preferences: Preferences,
    snapshot: Option<Snapshot>,
    pending_comparison: Option<Comparison>,
    pending_root: Option<String>,
    content: Option<Arc<FileContent>>,
    queue: ReviewQueue,
    all_files: bool,
    pending_all_files: bool,
    loaded_files: BTreeMap<String, Arc<FileContent>>,
    diff: Entity<DiffState>,
    path: Entity<InputState>,
    filter: Entity<InputState>,
    reference: Entity<InputState>,
    search: Entity<InputState>,
    line: Entity<InputState>,
    column: Entity<InputState>,
    comment: Entity<TextareaState>,
    edit_comment: Entity<TextareaState>,
    settings: Entity<TextareaState>,
    settings_form: Option<settings::SettingsForm>,
    focus: FocusHandle,
    subscriptions: Vec<Subscription>,
    loading: bool,
    error: Option<String>,
    notice: String,
    anchor: Option<DiffLinePosition>,
    editing: Option<String>,
    comment_anchor: Option<DiffLinePosition>,
    comment_fingerprint: Option<String>,
    pending_new: Option<Box<SavedView>>,
    saved_views: BTreeMap<String, SavedView>,
    pending_saves: Vec<tokio::task::JoinHandle<Result<(), String>>>,
    quick_index: usize,
    quick_input: Entity<InputState>,
    quick_visible: bool,
    quick_previous_focus: Option<FocusHandle>,
    palette_scroll: UniformListScrollHandle,
    palette_scroll_pending: bool,
    tree_scroll: UniformListScrollHandle,
    quick_scroll_pending: bool,
    collapsed: BTreeSet<String>,
    tree_focus: FocusHandle,
    resize_focus: [FocusHandle; 2],
    resize_drag: Option<(usize, f32, SidebarWidths)>,
    tree_cursor: Option<String>,
    tree_reveal_pending: bool,
    branches: Vec<String>,
    commits: Vec<Commit>,
    more_commits: bool,
    history_visible: bool,
    history_panel_bounds: Option<Bounds<Pixels>>,
    history_trigger_bounds: Option<Bounds<Pixels>>,
    commits_loading: bool,
    branches_visible: bool,
    search_query: String,
    search_fresh: bool,
    pending_location: Option<peer_core::navigation::SourceRange>,
    settings_visible: bool,
    recent_visible: bool,
    hits: Vec<presentation::SearchHit>,
    hit: usize,
    navigation_history: Vec<NavigationCheckpoint>,
    navigation: Option<NavigationResult>,
    navigation_file: Option<NavigationFile>,
    navigation_request: Option<String>,
}

// Checkpoints retain source identity, never a copy of an editable draft. Back
// reads the latest per-file draft (including successful-save tombstones).
struct NavigationCheckpoint {
    diff: DiffCheckpoint,
    content: Option<Arc<FileContent>>,
    all_files: bool,
    loaded_files: BTreeMap<String, Arc<FileContent>>,
    file: Option<NavigationFile>,
    anchor: Option<DiffLinePosition>,
    navigation: Option<NavigationResult>,
    target: Option<peer_core::navigation::SourceRange>,
    column: String,
}

fn source_byte(source: &str, position: Position) -> Option<usize> {
    let mut offset = 0;
    for (number, line) in source.split_inclusive('\n').enumerate() {
        if number == position.line as usize {
            let text = line.trim_end_matches(['\r', '\n']);
            let mut units = 0;
            for (byte, ch) in text.char_indices() {
                if units == position.character {
                    return Some(offset + byte);
                }
                units += ch.len_utf16() as u32;
            }
            return (units == position.character).then_some(offset + text.len());
        }
        offset += line.len();
    }
    (position.character == 0
        && (source.is_empty() || source.ends_with('\n'))
        && position.line as usize == source.split_inclusive('\n').count())
    .then_some(source.len())
}
fn input(placeholder: &str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    let placeholder = placeholder.to_owned();
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
}
fn side(side: DiffSide) -> &'static str {
    if side == DiffSide::Original {
        "old"
    } else {
        "new"
    }
}
fn diff_side(side: &str) -> DiffSide {
    if side == "old" {
        DiffSide::Original
    } else {
        DiffSide::Modified
    }
}
fn files(content: &FileContent, code: bool) -> Result<Vec<DiffFile>, String> {
    if content.deferred || content.kind != "text" {
        return Ok(vec![]);
    }
    if code || content.old_content == content.new_content {
        return Ok(vec![DiffFile::unchanged(
            content.path.clone(),
            content
                .new_content
                .as_deref()
                .or(content.old_content.as_deref())
                .unwrap_or(""),
        )]);
    }
    let patch = presentation::unified_patch(content);
    if patch.is_empty() {
        return Ok(vec![DiffFile::unchanged(content.path.clone(), "")]);
    }
    DiffFile::parse(&patch)
        .map(|files| {
            files
                .into_iter()
                .map(|f| f.with_display_options(content.tab_width as usize, false))
                .collect()
        })
        .map_err(|e| e.to_string())
}

async fn navigation_models(
    core: Arc<Inspector>,
    snapshot: String,
    side: String,
    document: String,
    force: bool,
    width: u32,
    whitespace: bool,
) -> Result<(NavigationFile, Vec<DiffFile>), String> {
    let file = core
        .read_navigation_file(&snapshot, &side, &document, force)
        .await
        .map_err(|e| e.message)?;
    tokio::task::spawn_blocking(move || {
        let models = file
            .content
            .as_ref()
            .map(|text| {
                vec![DiffFile::unchanged(file.path.clone(), text)
                    .with_display_options(file.tab_width.unwrap_or(width) as usize, whitespace)]
            })
            .unwrap_or_default();
        (file, models)
    })
    .await
    .map_err(|e| e.to_string())
}

impl Workspace {
    pub fn new(
        core: Arc<Inspector>,
        runtime: Arc<tokio::runtime::Runtime>,
        preferences: Preferences,
        initial: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_window_title("Peer");
        let path = input("Repository path", window, cx);
        let filter = input("Filter files…", window, cx);
        let reference = input("Branch or commit", window, cx);
        let search = input("Find in old/new source", window, cx);
        let line = input("Line", window, cx);
        let column = input("UTF-16 column (0-based)", window, cx);
        let comment = cx.new(|cx| TextareaState::new(window, cx).placeholder("Review comment"));
        let edit_comment = cx.new(|cx| TextareaState::new(window, cx).placeholder("Edit comment"));
        let settings = cx.new(|cx| TextareaState::new(window, cx).placeholder("Preferences JSON"));
        let diff = cx.new(|cx| {
            DiffState::new([], cx).with_mode(if preferences.diff_layout == "split" {
                DiffMode::Split
            } else {
                DiffMode::Unified
            })
        });
        let subscriptions = vec![cx.subscribe(&filter, |_, _, _: &InputEvent, cx| cx.notify())];
        let mut this = Self {
            settings_form: None,
            pending_jobs: Default::default(),
            core,
            runtime,
            gate: Arc::new(tokio::sync::Mutex::new(())),
            epoch: Arc::new(AtomicU64::new(0)),
            file_epoch: 0,
            themes: crate::themes::builtins(),
            feedback_pending: false,
            feedback_visible: true,
            clear_queue_request: None,
            pending_jump: None,
            preferences,
            snapshot: None,
            pending_comparison: None,
            pending_root: None,
            content: None,
            queue: ReviewQueue::default(),
            all_files: false,
            pending_all_files: false,
            loaded_files: BTreeMap::new(),
            diff,
            path,
            filter,
            reference,
            search,
            line,
            column,
            comment,
            edit_comment,
            settings,
            focus: cx.focus_handle(),
            subscriptions,
            loading: false,
            error: None,
            notice: String::new(),
            anchor: None,
            editing: None,
            comment_anchor: None,
            comment_fingerprint: None,
            pending_new: None,
            saved_views: BTreeMap::new(),
            pending_saves: vec![],
            quick_index: 0,
            quick_input: input("Type a filename…", window, cx),
            quick_visible: false,
            quick_previous_focus: None,
            palette_scroll: UniformListScrollHandle::new(),
            palette_scroll_pending: false,
            tree_scroll: UniformListScrollHandle::new(),
            quick_scroll_pending: false,
            collapsed: BTreeSet::new(),
            tree_focus: cx.focus_handle(),
            resize_focus: [cx.focus_handle(), cx.focus_handle()],
            resize_drag: None,
            tree_cursor: None,
            tree_reveal_pending: false,
            branches: vec![],
            commits: vec![],
            more_commits: false,
            history_visible: false,
            history_panel_bounds: None,
            history_trigger_bounds: None,
            commits_loading: false,
            branches_visible: false,
            search_query: String::new(),
            search_fresh: true,
            pending_location: None,
            settings_visible: false,
            recent_visible: false,
            hits: vec![],
            hit: 0,
            navigation_history: vec![],
            navigation: None,
            navigation_file: None,
            navigation_request: None,
        };
        this.subscriptions.push(cx.subscribe_in(
            &this.quick_input.clone(),
            window,
            |this, _, event: &InputEvent, w, c| match event {
                InputEvent::Change => {
                    this.quick_index = 0;
                    this.palette_scroll_pending = true;
                    c.notify();
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(file) = this.palette_files(c).get(this.quick_index) {
                        this.open_palette_file(file.id.clone(), w, c);
                    }
                }
                _ => {}
            },
        ));
        // Diff is RenderOnce around retained state. Observe it so post-layout
        // geometry and background presentation updates rebuild its headers too,
        // including explicit Back/Find moves that intentionally keep identity.
        this.subscriptions
            .push(cx.observe(&this.diff, |_, _, cx| cx.notify()));
        this.subscriptions.push(cx.subscribe_in(
            &this.diff.clone(),
            window,
            |this, _, event: &DiffEvent, w, c| {
                if let DiffEvent::ViewportFileChanged(path) = event {
                    if this.all_files
                        && this.navigation_file.is_none()
                        && this.diff.read(c).viewport_file_is_current(path)
                    {
                        this.activate_loaded(path, w, c);
                    }
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.filter.clone(),
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    this.quick_index = if this.filter.read(cx).value().is_empty() {
                        this.snapshot
                            .as_ref()
                            .and_then(|s| {
                                s.files.iter().position(|f| {
                                    this.tree_cursor.as_deref()
                                        == Some(format!("file:{}", f.path).as_str())
                                })
                            })
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    this.quick_scroll_pending = true;
                    this.tree_reveal_pending = true;
                    window.refresh();
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    let filter = this.filter.read(cx).value().to_lowercase();
                    let id = this
                        .snapshot
                        .as_ref()
                        .and_then(|s| {
                            s.files
                                .iter()
                                .filter(|f| f.path.to_lowercase().contains(&filter))
                                .nth(this.quick_index)
                        })
                        .map(|f| f.id.clone());
                    if let Some(id) = id {
                        this.select(id, false, window, cx);
                        this.diff.read(cx).focus_handle(cx).focus(window, cx);
                    }
                }
                _ => {}
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.path.clone(),
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.open(this.path.read(cx).value().to_string(), None, window, cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.comment.clone(),
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(
                    event,
                    InputEvent::PressEnter {
                        secondary: true,
                        ..
                    }
                ) {
                    this.save_new_comment(window, cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.edit_comment.clone(),
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(
                    event,
                    InputEvent::PressEnter {
                        secondary: true,
                        ..
                    }
                ) {
                    this.save_comment(window, cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.search.clone(),
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    this.hits.clear();
                    this.search_fresh = true;
                    this.search_query = this.search.read(cx).value().to_string();
                    if !this.search_query.is_empty() {
                        this.find(0, window, cx);
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { shift, .. } => {
                    this.find(if *shift { -1 } else { 1 }, window, cx)
                }
                _ => {}
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.line.clone(),
            window,
            |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.jump(cx);
                }
            },
        ));
        this.subscriptions
            .push(cx.observe_window_appearance(window, |this, window, cx| {
                if this.preferences.appearance.mode == "system" {
                    this.apply_theme(window, cx);
                }
            }));
        this.subscriptions.push(cx.on_app_quit(|this, cx| {
            this.cancel_navigation();
            this.remember(cx);
            let jobs = std::mem::take(this.pending_jobs.get_mut());
            let saves = std::mem::take(&mut this.pending_saves);
            async move {
                for job in jobs {
                    let _ = job.await;
                }
                for save in saves {
                    match save.await {
                        Ok(Ok(())) => {}
                        result => eprintln!("Session persistence failed: {result:?}"),
                    }
                }
            }
        }));
        window.on_window_should_close(cx, |_, cx| {
            cx.quit();
            false
        });
        this.apply_theme(window, cx);
        this.focus.focus(window, cx);
        if let Some(path) = initial {
            this.path
                .update(cx, |s, cx| s.set_value(path.clone(), window, cx));
            this.open(path, None, window, cx);
        }
        this
    }
    /// Tokio owns every core request. GPUI awaits only the oneshot, never block_on.
    fn task<T: Send + 'static>(
        &self,
        future: impl Future<Output = Result<T, String>> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, T, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let epoch = self.epoch.load(Ordering::SeqCst);
        let generation = self.file_epoch;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_jobs
            .borrow_mut()
            .retain(|job| !job.is_finished());
        self.pending_jobs
            .borrow_mut()
            .push(self.runtime.spawn(async move {
                let result = future.await;
                let _ = tx.send(result);
            }));
        cx.spawn_in(window, async move |this, cx| {
            let result = rx
                .await
                .unwrap_or_else(|_| Err("Background task stopped unexpectedly".into()));
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(value) => done(this, value, window, cx),
                    Err(error) => {
                        if this.epoch.load(Ordering::SeqCst) == epoch
                            && this.file_epoch == generation
                        {
                            this.loading = false;
                            this.notice = "Action failed; see error above".into();
                            this.error = Some(error);
                            this.commits_loading = false;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn draft_key(&self) -> Option<String> {
        self.content.as_ref().map(|f| {
            format!(
                "draft-v2:{}:{}",
                self.snapshot
                    .as_ref()
                    .map(|s| s.queue_id.as_str())
                    .unwrap_or(""),
                f.path
            )
        })
    }
    fn comment_view(&self, cx: &App) -> SavedView {
        let mut pending_new = self.pending_new.clone();
        if let Some(view) = pending_new.as_mut() {
            view.body = self.comment.read(cx).value().to_string();
        }
        SavedView {
            body: if self.editing.is_some() {
                self.edit_comment.read(cx).value().to_string()
            } else {
                self.comment.read(cx).value().to_string()
            },
            fingerprint: self
                .comment_fingerprint
                .clone()
                .or_else(|| self.content.as_ref().map(|f| f.fingerprint.clone())),
            pending_new,
            side: self.comment_anchor.as_ref().map(|a| side(a.side()).into()),
            line: self.comment_anchor.as_ref().map(|a| a.line()),
            editing: self.editing.clone(),
            ..Default::default()
        }
    }
    fn saved_comment_view(&self) -> Option<SavedView> {
        let key = self.draft_key()?;
        if let Some(view) = self.saved_views.get(&key) {
            return Some(view.clone());
        }
        // Read the fingerprint-keyed format shipped by the first native version.
        let fingerprint = &self.content.as_ref()?.fingerprint;
        let key = format!(
            "{}:{}",
            self.snapshot.as_ref()?.queue_id,
            self.content.as_ref()?.path
        );
        legacy_draft(&self.saved_views, &key, fingerprint)
    }

    fn restore_comment(&mut self, view: SavedView, window: &mut Window, cx: &mut Context<Self>) {
        let path = self
            .content
            .as_ref()
            .map(|f| f.path.clone())
            .unwrap_or_default();
        self.comment_anchor = view.line.map(|line| {
            DiffLinePosition::new(path, diff_side(view.side.as_deref().unwrap_or("new")), line)
        });
        self.comment_fingerprint = view.fingerprint;
        self.editing = view.editing;
        self.pending_new = view.pending_new;
        if self.editing.is_some() {
            self.edit_comment
                .update(cx, |s, cx| s.set_value(view.body, window, cx));
            let body = self
                .pending_new
                .as_ref()
                .map(|v| v.body.clone())
                .unwrap_or_default();
            self.comment
                .update(cx, |s, cx| s.set_value(body, window, cx));
        } else {
            self.comment
                .update(cx, |s, cx| s.set_value(view.body, window, cx));
        }
    }
    fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self
            .comment_view(cx)
            .pending_new
            .map(|v| *v)
            .unwrap_or_default();
        self.restore_comment(view, window, cx);
        self.remember(cx);
        cx.notify();
    }
    fn begin_comment(
        &mut self,
        position: Option<DiffLinePosition>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.navigation_file.is_some() || self.loading {
            return;
        }
        if let Some(pending) = &mut self.pending_new {
            pending.side = position.as_ref().map(|a| side(a.side()).into());
            pending.line = position.as_ref().map(|a| a.line());
            pending.fingerprint = self.content.as_ref().map(|f| f.fingerprint.clone());
        }
        self.error = None;
        self.comment_anchor = position;
        self.comment_fingerprint = self.content.as_ref().map(|f| f.fingerprint.clone());
        self.feedback_visible = true;
        self.comment.update(cx, |s, cx| s.focus(window, cx));
        self.remember(cx);
        cx.notify();
    }
    fn remember(&mut self, cx: &mut Context<Self>) {
        // Frozen navigation sources cannot replace the review draft or its source anchor.
        if self.navigation_file.is_some() {
            return;
        }
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        let root = snapshot.repository.root.clone();
        let selected_path = self.content.as_ref().map(|f| f.path.clone());
        let comparison = snapshot.comparison.clone();
        if comparison.view == "changes" {
            self.preferences
                .sessions
                .entry(root.clone())
                .or_insert_with(|| Session {
                    comparison: comparison.clone(),
                    selected_path: selected_path.clone(),
                    scroll_top: 0.,
                    last_changes: None,
                })
                .last_changes = Some(comparison.clone());
        }
        let viewport = self
            .diff
            .read(cx)
            .viewport_anchor()
            .filter(|(p, _)| selected_path.as_deref() == Some(p.path().as_str()));
        let mut view = self.comment_view(cx);
        view.scroll_line = viewport.as_ref().map(|(a, _)| a.line());
        view.scroll_side = viewport.as_ref().map(|(a, _)| side(a.side()).into());
        view.scroll_offset = viewport.map(|(_, offset)| offset).unwrap_or_default();
        view.horizontal = self.diff.read(cx).horizontal_offset();
        let key = self.draft_key();
        if let Some(key) = &key {
            self.saved_views.insert(key.clone(), view.clone());
        }
        let core = self.core.clone();
        let gate = self.gate.clone();
        // Preserve UI transition order even when Tokio schedules later writes
        // first. Every file's draft is retained; only chaining session writes
        // avoids an older selectedPath overwriting the restored identity.
        let all_files_preference = self.preferences.all_files;
        let previous_saves = std::mem::take(&mut self.pending_saves);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_saves.push(self.runtime.spawn(async move {
            for save in previous_saves {
                let _ = save.await;
            }
            let _guard = gate.lock().await;
            let result = core.store.update(|data| {
                data.preferences.all_files = all_files_preference;
                let legacy_top = data
                    .preferences
                    .sessions
                    .get(&root)
                    .map(|s| s.scroll_top)
                    .unwrap_or(0.);
                data.preferences.sessions.insert(
                    root.clone(),
                    Session {
                        comparison: comparison.clone(),
                        selected_path,
                        scroll_top: legacy_top,
                        last_changes: if comparison.view == "changes" {
                            Some(comparison)
                        } else {
                            data.preferences
                                .sessions
                                .get(&root)
                                .and_then(|s| s.last_changes.clone())
                        },
                    },
                );
                if let Some(key) = key {
                    data.native_state
                        .insert(key, serde_json::to_value(view).map_err(|e| e.to_string())?);
                }
                Ok(())
            });
            if let Err(error) = &result {
                eprintln!("Cannot persist native session: {error}");
            }
            let _ = tx.send(result.clone());
            result
        }));
        cx.spawn(async move |this, cx| {
            if let Ok(Err(error)) = rx.await {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("Cannot save session: {error}"));
                    cx.notify();
                });
            }
        })
        .detach();
    }
    fn cancel_navigation(&mut self) {
        if let Some(request) = self.navigation_request.take() {
            self.core.cancel_navigation(&request);
        }
        self.navigation = None;
    }
    fn open(
        &mut self,
        path: String,
        comparison: Option<Comparison>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.remember(cx);
        self.clear_queue_request = None;
        self.pending_jump = None;
        self.commits_loading = false;
        self.history_visible = false;
        self.pending_root = Some(path.clone());
        self.pending_comparison = comparison.clone();
        self.path
            .update(cx, |s, cx| s.set_value(path.clone(), window, cx));
        self.cancel_navigation();
        let epoch = self.epoch.fetch_add(1, Ordering::SeqCst) + 1;
        self.file_epoch += 1;
        self.all_files = false;
        self.loaded_files.clear();
        self.loading = true;
        self.error = None;
        self.content = None;
        self.snapshot = None;
        self.anchor = None;
        self.navigation_history.clear();
        self.navigation_file = None;
        self.diff.update(cx, |s, cx| s.set_files([], cx));
        let core = self.core.clone();
        let gate = self.gate.clone();
        let clock = self.epoch.clone();
        let saves = std::mem::take(&mut self.pending_saves);
        self.task(
            async move {
                for save in saves {
                    save.await.map_err(|e| e.to_string())??;
                }
                let _guard = gate.lock().await;
                if clock.load(Ordering::SeqCst) != epoch {
                    return Ok(None);
                }
                let snapshot = core.open_repository(&path, comparison).await?;
                let queue = core.queue(&snapshot.id).await?;
                let data = core.store.read()?;
                let preferences = data.preferences;
                let saved_views = data.native_state;
                let themes = data.themes;
                let branches = core.branches(&snapshot.id).await?;
                Ok(Some((
                    snapshot,
                    queue,
                    preferences,
                    branches,
                    saved_views,
                    themes,
                )))
            },
            window,
            cx,
            move |this, result, window, cx| {
                if this.epoch.load(Ordering::SeqCst) != epoch {
                    return;
                }
                this.loading = false;
                if let Some((snapshot, queue, preferences, branches, saved_views, themes)) = result
                {
                    let selected = preferences
                        .sessions
                        .get(&snapshot.repository.root)
                        .and_then(|s| s.selected_path.clone());
                    this.path.update(cx, |s, cx| {
                        s.set_value(snapshot.repository.root.clone(), window, cx)
                    });
                    this.themes = crate::themes::builtins();
                    this.themes.extend(themes);
                    this.saved_views = saved_views
                        .into_iter()
                        .filter_map(|(key, value)| {
                            serde_json::from_value(value).ok().map(|view| (key, view))
                        })
                        .collect();
                    this.queue = queue;
                    this.preferences = preferences;
                    this.branches = branches;
                    this.commits.clear();
                    this.pending_comparison = Some(snapshot.comparison.clone());
                    this.pending_all_files =
                        this.preferences.all_files && snapshot.comparison.view == "changes";
                    this.snapshot = Some(snapshot);
                    this.apply_theme(window, cx);
                    let first = this
                        .snapshot
                        .as_ref()
                        .and_then(|s| {
                            selected
                                .and_then(|p| s.files.iter().find(|f| f.path == p))
                                .or(s.files.first())
                        })
                        .map(|f| f.id.clone());
                    if let Some(id) = first {
                        this.select(id, false, window, cx);
                    } else {
                        this.remember(cx);
                    }
                }
            },
        );
        cx.notify();
    }
    fn palette_files(&self, cx: &App) -> Vec<FileEntry> {
        let query = self.quick_input.read(cx).value().to_lowercase();
        self.snapshot
            .as_ref()
            .map(|s| {
                s.files
                    .iter()
                    .filter(|f| f.path.to_lowercase().contains(&query))
                    .take(100)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.quick_visible = false;
        if let Some(focus) = self.quick_previous_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn open_palette_file(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.close_palette(window, cx);
        // A global file jump must never leave its selected row hidden by a
        // previous sidebar query. The palette owns a separate query input.
        self.filter.update(cx, |s, cx| s.set_value("", window, cx));
        self.select(id, false, window, cx);
        self.diff.read(cx).focus_handle(cx).focus(window, cx);
    }
    fn tree_rows(&self, cx: &App) -> Vec<file_tree::Row> {
        file_tree::rows(
            self.snapshot
                .as_ref()
                .map(|s| s.files.as_slice())
                .unwrap_or(&[]),
            &self.filter.read(cx).value(),
            &self.collapsed,
        )
    }
    fn reveal_tree(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        file_tree::reveal(path, &mut self.collapsed);
        self.tree_cursor = Some(format!("file:{path}"));
        let mut query = self.filter.read(cx).value().to_lowercase();
        if !path.to_lowercase().contains(&query) {
            self.filter.update(cx, |s, cx| s.set_value("", window, cx));
            query.clear();
        }
        self.quick_index = self
            .snapshot
            .as_ref()
            .and_then(|s| {
                s.files
                    .iter()
                    .filter(|f| f.path.to_lowercase().contains(&query))
                    .position(|f| f.path == path)
            })
            .unwrap_or(0);
        self.tree_reveal_pending = true;
    }
    fn tree_click(
        &mut self,
        path: &str,
        folder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tree_focus.focus(window, cx);
        self.tree_cursor = Some(format!("{}:{path}", if folder { "dir" } else { "file" }));
        if folder {
            if !self.collapsed.remove(path) {
                self.collapsed.insert(path.into());
            }
        } else if let Some(file) = self
            .snapshot
            .as_ref()
            .and_then(|s| s.files.iter().find(|f| f.path == path))
        {
            self.select(file.id.clone(), false, window, cx);
        }
        cx.notify();
    }
    fn tree_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.tree_rows(cx);
        if rows.is_empty() {
            return;
        }
        let ix = rows
            .iter()
            .position(|r| Some(&r.key()) == self.tree_cursor.as_ref())
            .unwrap_or(0);
        let row = &rows[ix];
        let mut target = ix;
        match key {
            "up" => target = ix.saturating_sub(1),
            "down" => target = (ix + 1).min(rows.len() - 1),
            "home" => target = 0,
            "end" => target = rows.len() - 1,
            "left" if row.file.is_none() && !self.collapsed.contains(&row.path) => {
                self.collapsed.insert(row.path.clone());
            }
            "left" => {
                if let Some((parent, _)) = row.path.rsplit_once('/') {
                    if let Some(i) = rows
                        .iter()
                        .position(|r| r.path == parent && r.file.is_none())
                    {
                        target = i;
                    }
                }
            }
            "right" if row.file.is_none() => {
                if !self.collapsed.remove(&row.path) {
                    target = (ix + 1).min(rows.len() - 1);
                }
            }
            "enter" if row.file.is_none() => {
                if !self.collapsed.remove(&row.path) {
                    self.collapsed.insert(row.path.clone());
                }
            }
            _ => {}
        }
        let chosen = &rows[target];
        self.tree_cursor = Some(chosen.key());
        self.tree_reveal_pending = true;
        if let Some(file) = &chosen.file {
            if (key != "left" && key != "right") || target != ix {
                self.select(file.id.clone(), false, window, cx);
            }
        }
        window.refresh();
        cx.notify();
    }
    fn select(&mut self, id: String, force: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self
            .snapshot
            .as_ref()
            .and_then(|s| s.files.iter().find(|f| f.id == id))
            .map(|f| f.path.clone())
        {
            self.reveal_tree(&path, window, cx);
        }
        if !force
            && !self.all_files
            && self.navigation_file.is_none()
            && self.content.as_ref().is_some_and(|f| f.file_id == id)
        {
            cx.notify();
            return;
        }
        if !force
            && (self.navigation_file.is_some()
                || self.content.as_ref().is_some_and(|f| f.file_id != id))
        {
            self.push_navigation_history(cx);
        }
        if self.all_files && !force && self.navigation_file.is_none() {
            if let Some(path) = self
                .loaded_files
                .values()
                .find(|f| f.file_id == id)
                .map(|f| f.path.clone())
            {
                self.activate_loaded(&path, window, cx);
                self.diff.update(cx, |s, cx| s.scroll_to_file(&path, cx));
                return;
            }
        }
        self.all_files = false;
        self.loaded_files.clear();
        self.remember(cx);
        self.cancel_navigation();
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        let snapshot_id = snapshot.id.clone();
        let code = snapshot.comparison.view == "code";
        self.file_epoch += 1;
        let generation = self.file_epoch;
        let epoch = self.epoch.load(Ordering::SeqCst);
        self.content = None;
        self.navigation_file = None;
        self.anchor = None;
        self.editing = None;
        self.comment_anchor = None;
        self.comment_fingerprint = None;
        self.pending_new = None;
        self.hits.clear();
        self.error = None;
        self.loading = true;
        self.comment.update(cx, |s, cx| s.set_value("", window, cx));
        self.diff.update(cx, |s, cx| s.set_files([], cx));
        let whitespace = self.preferences.show_whitespace;
        let core = self.core.clone();
        self.task(
            async move {
                let content = core.read_file(&snapshot_id, &id, force).await?;
                let result = tokio::task::spawn_blocking(move || {
                    let files = files(&content, code)?
                        .into_iter()
                        .map(|f| f.with_display_options(content.tab_width as usize, whitespace))
                        .collect::<Vec<_>>();
                    Ok::<_, String>((Arc::new(content), files))
                })
                .await
                .map_err(|e| e.to_string())??;
                Ok(result)
            },
            window,
            cx,
            move |this, (content, files), window, cx| {
                if this.file_epoch != generation || this.epoch.load(Ordering::SeqCst) != epoch {
                    return;
                }
                this.loading = false;
                this.diff.update(cx, |s, cx| s.set_files(files, cx));
                this.content = Some(content);
                if let Some(view) = this.saved_comment_view() {
                    this.restore_comment(view.clone(), window, cx);
                    this.diff
                        .update(cx, |s, cx| s.restore_horizontal(view.horizontal, cx));
                    let path = this
                        .content
                        .as_ref()
                        .map(|f| f.path.clone())
                        .unwrap_or_default();
                    this.comment_anchor = view.line.map(|line| {
                        DiffLinePosition::new(
                            path.clone(),
                            diff_side(view.side.as_deref().unwrap_or("new")),
                            line,
                        )
                    });
                    if let Some(line) = view.scroll_line {
                        this.diff.update(cx, |s, cx| {
                            s.restore_viewport(
                                DiffLinePosition::new(
                                    path,
                                    diff_side(view.scroll_side.as_deref().unwrap_or("new")),
                                    line,
                                ),
                                view.scroll_offset,
                                cx,
                            )
                        });
                    }
                } else if let Some(top) = this
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| this.preferences.sessions.get(&snapshot.repository.root))
                    .map(|session| session.scroll_top)
                {
                    this.diff
                        .update(cx, |s, cx| s.restore_legacy_scroll(top as f32, cx));
                }
                if let Some(position) = this.pending_jump.take().filter(|p| {
                    this.content
                        .as_ref()
                        .is_some_and(|f| f.path == p.path().as_str())
                }) {
                    this.diff
                        .update(cx, |s, cx| s.scroll_to_line(position.clone(), cx));
                    this.anchor = Some(position);
                }
                this.remember(cx);
                if this.pending_all_files {
                    this.pending_all_files = false;
                    this.load_all(window, cx);
                }
            },
        );
        cx.notify();
    }
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = &self.snapshot {
            self.open(
                s.repository.root.clone(),
                Some(s.comparison.clone()),
                window,
                cx,
            );
        } else {
            self.open(self.path.read(cx).value().to_string(), None, window, cx);
        }
    }
    fn compare(&mut self, mode: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self
            .snapshot
            .as_ref()
            .map(|s| s.repository.root.clone())
            .or_else(|| self.pending_root.clone())
        else {
            return;
        };
        let comparison = Comparison {
            view: if mode == "code" { "code" } else { "changes" }.into(),
            mode: if mode == "code" { "all" } else { mode }.into(),
            base_ref: if ["branch", "commit"].contains(&mode) {
                Some(self.reference.read(cx).value().to_string())
            } else {
                None
            },
            include_untracked: self
                .snapshot
                .as_ref()
                .map(|s| &s.comparison)
                .or(self.pending_comparison.as_ref())
                .map(|c| {
                    if c.mode == "commit" {
                        Comparison::default().include_untracked
                    } else {
                        c.include_untracked
                    }
                })
                .unwrap_or(true),
        };
        self.open(path, Some(comparison), window, cx);
    }
    fn resume_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let path = snapshot.repository.root.clone();
        let comparison = self
            .preferences
            .sessions
            .get(&path)
            .and_then(|s| s.last_changes.clone())
            .filter(|c| c.view == "changes")
            .unwrap_or_default();
        self.open(path, Some(comparison), window, cx);
    }
    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open repository".into()),
        });
        cx.spawn_in(window, async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.first() {
                    let path = path.to_string_lossy().into_owned();
                    let _ =
                        this.update_in(cx, |this, window, cx| this.open(path, None, window, cx));
                }
            }
            Ok(Err(error)) => {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(error.to_string());
                    cx.notify();
                });
            }
            _ => {}
        })
        .detach();
    }
    fn update_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let id = snapshot.id.clone();
        let core = self.core.clone();
        self.task(
            async move { core.queue(&id).await.map(|q| (id, q)) },
            window,
            cx,
            |this, (id, queue), _, _| {
                if this.snapshot.as_ref().is_some_and(|s| s.id == id) {
                    this.queue = queue;
                }
            },
        );
    }
    fn save_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_comment_kind(self.editing.is_some(), window, cx);
    }
    fn save_new_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_comment_kind(false, window, cx);
    }
    fn save_comment_kind(&mut self, edit: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.feedback_pending || self.loading || self.navigation_file.is_some() {
            return;
        }
        let (Some(snapshot), Some(content)) = (&self.snapshot, &self.content) else {
            return;
        };
        let view = self.comment_view(cx);
        let submitted = if !edit && self.editing.is_some() {
            view.pending_new.as_deref().cloned().unwrap_or_default()
        } else {
            view
        };
        let body = submitted.body.clone();
        if body.trim().is_empty() {
            return;
        }
        if submitted.editing.is_none()
            && submitted
                .fingerprint
                .as_ref()
                .is_some_and(|fp| fp != &content.fingerprint)
        {
            self.error = Some(
                "Draft source changed; explicitly choose its new comment location before saving"
                    .into(),
            );
            cx.notify();
            return;
        }
        let queue = snapshot.queue_id.clone();
        let editing = submitted.editing.clone();
        let restored = if editing.is_some() {
            submitted
                .pending_new
                .as_deref()
                .cloned()
                .unwrap_or_default()
        } else {
            SavedView::default()
        };
        let persisted = restored.clone();
        let persisted_submission = submitted.clone();
        let key = self.draft_key();
        let generation = self.file_epoch;
        let comment = NewComment {
            snapshot_id: snapshot.id.clone(),
            file_id: content.file_id.clone(),
            fingerprint: content.fingerprint.clone(),
            body: body.clone(),
            side: if snapshot.comparison.view == "code" {
                None
            } else {
                submitted.side.clone()
            },
            line: submitted.line.map(|line| line as u32),
        };
        let core = self.core.clone();
        let gate = self.gate.clone();
        let saved_key = key.clone();
        self.feedback_pending = true;
        self.task(
            async move {
                Ok(async move {
                    let _guard = gate.lock().await;
                    if let Some(id) = editing {
                        core.update_comment(&queue, &id, &body)?;
                    } else {
                        core.add_comment(comment).await?;
                    }
                    core.store.update(|d| {
                        if let Some(key) = saved_key {
                            let current = d
                                .native_state
                                .get(&key)
                                .and_then(|v| serde_json::from_value::<SavedView>(v.clone()).ok());
                            let next = current.map_or_else(
                                || Some(persisted.clone()),
                                |v| v.after_submission(&persisted_submission, &persisted),
                            );
                            if let Some(next) = next {
                                d.native_state.insert(
                                    key,
                                    serde_json::to_value(next).map_err(|e| e.to_string())?,
                                );
                            }
                        }
                        Ok(())
                    })
                }
                .await)
            },
            window,
            cx,
            move |this, result: Result<(), String>, window, cx| {
                this.feedback_pending = false;
                if let Err(error) = result {
                    this.error = Some(error);
                    return;
                }
                if let Some(key) = &key {
                    let next = this.saved_views.get(key).map_or_else(
                        || Some(restored.clone()),
                        |v| v.after_submission(&submitted, &restored),
                    );
                    if let Some(next) = next {
                        this.saved_views.insert(key.clone(), next);
                    }
                }
                if this.draft_key() == key {
                    if let Some(next) = this
                        .comment_view(cx)
                        .after_submission(&submitted, &restored)
                    {
                        this.restore_comment(next, window, cx);
                        this.remember(cx);
                    }
                }
                if this.file_epoch == generation {
                    this.remember(cx);
                }
                // A file switch can queue a draft write behind the comment save.
                // Drain those writes before conditionally removing the submitted draft.
                let saves = std::mem::take(&mut this.pending_saves);
                let core = this.core.clone();
                let gate = this.gate.clone();
                this.task(
                    async move {
                        for save in saves {
                            save.await.map_err(|e| e.to_string())??;
                        }
                        let _guard = gate.lock().await;
                        core.store.update(|data| {
                            if let Some(key) = key {
                                if let Some(next) = data
                                    .native_state
                                    .get(&key)
                                    .and_then(|v| {
                                        serde_json::from_value::<SavedView>(v.clone()).ok()
                                    })
                                    .and_then(|v| v.after_submission(&submitted, &restored))
                                {
                                    data.native_state.insert(
                                        key,
                                        serde_json::to_value(next).map_err(|e| e.to_string())?,
                                    );
                                }
                            }
                            Ok(())
                        })
                    },
                    window,
                    cx,
                    |_, (), _, _| {},
                );
                this.update_queue(window, cx);
            },
        );
    }
    fn delete_comment(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let queue = snapshot.queue_id.clone();
        let core = self.core.clone();
        let gate = self.gate.clone();
        let deleted = id.clone();
        self.task(
            async move {
                let _guard = gate.lock().await;
                core.delete_comments(&queue, Some(&id))
            },
            window,
            cx,
            move |this, (), window, cx| {
                if this.editing.as_ref() == Some(&deleted) {
                    this.cancel_edit(window, cx);
                }
                this.update_queue(window, cx);
            },
        );
    }
    fn jump_comment(
        &mut self,
        comment: peer_core::Comment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if comment.outdated {
            self.notice =
                "Original source changed; the saved line context is retained below the comment."
                    .into();
            cx.notify();
            return;
        }
        let entry = self
            .snapshot
            .as_ref()
            .and_then(|s| s.files.iter().find(|f| f.path == comment.path))
            .cloned();
        let Some(entry) = entry else {
            self.notice =
                "The original file is no longer present; saved context is retained.".into();
            cx.notify();
            return;
        };
        self.reveal_tree(&comment.path, window, cx);
        if self.all_files && self.loaded_files.contains_key(&comment.path) {
            self.activate_loaded(&comment.path, window, cx);
        }
        let position = DiffLinePosition::new(
            comment.path.clone(),
            diff_side(comment.anchor.side.as_deref().unwrap_or("new")),
            comment.anchor.line.unwrap_or(1) as usize,
        );
        if self
            .content
            .as_ref()
            .is_some_and(|f| f.path == comment.path)
            && self.navigation_file.is_none()
        {
            self.diff
                .update(cx, |s, cx| s.scroll_to_line(position.clone(), cx));
            self.anchor = Some(position);
        } else {
            self.pending_jump = Some(position);
            self.select(entry.id, false, window, cx);
        }
        cx.notify();
    }
    fn clear_review_comments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(queue) = self.clear_queue_request.take() else {
            return;
        };
        let core = self.core.clone();
        let gate = self.gate.clone();
        self.task(
            async move {
                let _guard = gate.lock().await;
                core.delete_comments(&queue, None)
            },
            window,
            cx,
            |this, (), w, c| {
                if this.editing.is_some() {
                    this.cancel_edit(w, c);
                }
                this.update_queue(w, c);
            },
        );
    }
    fn reviewed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(content) = &self.content else {
            return;
        };
        let reviewed = self.queue.reviewed.get(&content.path) != Some(&content.fingerprint);
        self.reviewed_file(content.clone(), reviewed, window, cx);
    }
    fn reviewed_file(
        &mut self,
        content: Arc<FileContent>,
        reviewed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let (id, file, fingerprint) = (
            snapshot.id.clone(),
            content.file_id.clone(),
            content.fingerprint.clone(),
        );
        let core = self.core.clone();
        let gate = self.gate.clone();
        self.task(
            async move {
                let _guard = gate.lock().await;
                core.set_reviewed(&id, &file, &fingerprint, reviewed).await
            },
            window,
            cx,
            |this, (), window, cx| this.update_queue(window, cx),
        );
    }
    fn export(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let id = snapshot.id.clone();
        let core = self.core.clone();
        self.task(
            async move { core.export(&id).await },
            window,
            cx,
            move |this, text, window, cx| {
                if !save {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    this.notice = "Feedback copied".into();
                    return;
                }
                let receiver =
                    cx.prompt_for_new_path(std::path::Path::new("."), Some("feedback.json"));
                cx.spawn_in(window, async move |this, cx| {
                    let response = receiver.await;
                    if let Err(error) = &response {
                        let _ = this.update(cx, |this, cx| {
                            this.error = Some(format!("Save dialog unavailable: {error}"));
                            cx.notify();
                        });
                    }
                    if let Ok(Err(error)) = &response {
                        let _ = this.update(cx, |this, cx| {
                            this.error = Some(format!("Save dialog: {error}"));
                            cx.notify();
                        });
                    }
                    if let Ok(Ok(Some(path))) = response {
                        let _ = this.update_in(cx, |this, window, cx| {
                            this.task(
                                async move {
                                    tokio::task::spawn_blocking(move || {
                                        std::fs::write(path, text).map_err(|e| e.to_string())
                                    })
                                    .await
                                    .map_err(|e| e.to_string())?
                                },
                                window,
                                cx,
                                |this, (), _, _| this.notice = "Feedback saved".into(),
                            );
                        });
                    }
                })
                .detach();
            },
        );
    }
    fn find(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(content) = self.content.clone() else {
            return;
        };
        let query = self.search.read(cx).value().to_string();
        let generation = self.file_epoch;
        let epoch = self.epoch.load(Ordering::SeqCst);
        let navigation_file = self.navigation_file.clone();
        let path = navigation_file
            .as_ref()
            .map(|f| f.path.clone())
            .unwrap_or_else(|| content.path.clone());
        let check = query.clone();
        self.task(
            async move {
                tokio::task::spawn_blocking(move || {
                    if let Some(file) = navigation_file {
                        let mut visible = (*content).clone();
                        visible.path = file.path;
                        visible.old_content = None;
                        visible.new_content = file.content;
                        presentation::search(&visible, &query)
                    } else {
                        presentation::search(&content, &query)
                    }
                })
                .await
                .map_err(|e| e.to_string())
            },
            window,
            cx,
            move |this, hits, _, cx| {
                if this.file_epoch != generation
                    || this.epoch.load(Ordering::SeqCst) != epoch
                    || this.search.read(cx).value().as_str() != check
                {
                    return;
                }
                if hits.is_empty() {
                    this.notice = "No matches".into();
                    this.hits = hits;
                    return;
                }
                if delta == 0 {
                    this.hits = hits;
                    cx.notify();
                    return;
                }
                if hits != this.hits || this.search_fresh {
                    this.hit = if delta < 0 { hits.len() - 1 } else { 0 };
                    this.search_fresh = false;
                } else {
                    this.hit = (this.hit as isize + delta).rem_euclid(hits.len() as isize) as usize;
                }
                let hit = &hits[this.hit];
                let position = DiffLinePosition::new(path, diff_side(hit.side), hit.line as usize);
                this.push_navigation_history(cx);
                this.anchor = Some(position.clone());
                this.diff.update(cx, |s, cx| s.scroll_to_line(position, cx));
                this.notice = format!(
                    "Match {}/{} — {} line {}, UTF-16 column {}",
                    this.hit + 1,
                    hits.len(),
                    hit.side,
                    hit.line,
                    hit.utf16
                );
                this.hits = hits;
            },
        );
    }
    fn jump(&mut self, cx: &mut Context<Self>) {
        let Some(content) = &self.content else { return };
        let Ok(line) = self.line.read(cx).value().parse::<usize>() else {
            self.error = Some("Enter a positive source line".into());
            return;
        };
        if line == 0 {
            return;
        }
        let selected_side =
            self.anchor
                .as_ref()
                .map(|p| p.side())
                .unwrap_or(if content.new_content.is_some() {
                    DiffSide::Modified
                } else {
                    DiffSide::Original
                });
        let selected_side = if self.navigation_file.is_some() {
            DiffSide::Modified
        } else {
            selected_side
        };
        let source = if let Some(file) = &self.navigation_file {
            &file.content
        } else if selected_side == DiffSide::Original {
            &content.old_content
        } else {
            &content.new_content
        };
        if !source.as_ref().is_some_and(|s| line <= s.lines().count()) {
            self.error = Some("Line is outside this source".into());
            return;
        }
        let path = self
            .navigation_file
            .as_ref()
            .map(|f| f.path.clone())
            .unwrap_or_else(|| content.path.clone());
        let position = DiffLinePosition::new(path, selected_side, line);
        self.push_navigation_history(cx);
        self.anchor = Some(position.clone());
        self.diff.update(cx, |s, cx| s.scroll_to_line(position, cx));
        cx.notify();
    }
    fn push_navigation_history(&mut self, cx: &App) {
        self.navigation_history.push(NavigationCheckpoint {
            diff: self.diff.read(cx).checkpoint(cx),
            content: self.content.clone(),
            all_files: self.all_files,
            loaded_files: self.loaded_files.clone(),
            file: self.navigation_file.clone(),
            anchor: self.anchor.clone(),
            navigation: self.navigation.clone(),
            target: self.pending_location.clone(),
            column: self.column.read(cx).value().to_string(),
        });
        if self.navigation_history.len() > 100 {
            self.navigation_history.remove(0);
        }
    }
    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(checkpoint) = self.navigation_history.pop() {
            self.remember(cx);
            self.file_epoch += 1;
            self.cancel_navigation();
            self.loading = false;
            self.pending_jump = None;
            self.content = checkpoint.content;
            self.all_files = checkpoint.all_files;
            self.loaded_files = checkpoint.loaded_files;
            self.navigation_file = checkpoint.file;
            self.notice = self
                .navigation_file
                .as_ref()
                .map(|f| f.label.clone())
                .unwrap_or_else(|| "Returned to review source".into());
            self.anchor = checkpoint.anchor;
            self.navigation = checkpoint.navigation;
            self.pending_location = checkpoint.target;
            self.column
                .update(cx, |s, cx| s.set_value(checkpoint.column, window, cx));
            self.hits.clear();
            self.search_fresh = true;
            self.restore_comment(self.saved_comment_view().unwrap_or_default(), window, cx);
            self.diff
                .update(cx, |s, cx| s.restore_checkpoint(checkpoint.diff, cx));
            if let Some(path) = self.content.as_ref().map(|f| f.path.clone()) {
                self.reveal_tree(&path, window, cx);
            }
            self.remember(cx);
            cx.notify();
        }
    }
    fn load_commits(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.commits_loading {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let id = snapshot.id.clone();
        let offset = self.commits.len() as u32;
        self.commits_loading = true;
        let core = self.core.clone();
        let epoch = self.epoch.load(Ordering::SeqCst);
        self.task(
            async move { core.commits(&id, offset, 50).await },
            window,
            cx,
            move |this, page, _, _| {
                if this.epoch.load(Ordering::SeqCst) == epoch {
                    this.commits_loading = false;
                    this.more_commits = page.has_more;
                    for commit in page.commits {
                        if !this.commits.iter().any(|c| c.id == commit.id) {
                            this.commits.push(commit);
                        }
                    }
                    this.notice = format!("{} commits loaded", this.commits.len());
                }
            },
        );
    }
    fn apply_theme(&self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = match self.preferences.appearance.mode.as_str() {
            "dark" => ThemeMode::Dark,
            "light" => ThemeMode::Light,
            _ => {
                if matches!(
                    window.appearance(),
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                ) {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                }
            }
        };
        Theme::change(mode, Some(window), cx);
        let id = if mode == ThemeMode::Dark {
            &self.preferences.appearance.dark_theme
        } else {
            &self.preferences.appearance.light_theme
        };
        if let Some(theme) = self.themes.iter().find(|t| &t.id == id) {
            crate::themes::apply(theme, self.preferences.code_font_size, cx);
        }
    }
    fn save_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let preferences = match self
            .settings_form
            .as_ref()
            .ok_or_else(|| "Settings are not open".to_string())
            .and_then(|f| f.value(&self.settings, cx))
        {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(e);
                cx.notify();
                return;
            }
        };
        let core = self.core.clone();
        let gate = self.gate.clone();
        self.task(
            async move {
                let _guard = gate.lock().await;
                let mut preferences = preferences;
                let latest = core.store.read()?.preferences;
                preferences.sessions = latest.sessions;
                preferences.recent_repositories = latest.recent_repositories;
                preferences.last_repository = latest.last_repository;
                core.save_preferences(preferences.clone())?;
                Ok(preferences)
            },
            window,
            cx,
            |this, preferences, window, cx| {
                this.preferences = preferences;
                this.settings_visible = false;
                this.focus.focus(window, cx);
                this.apply_theme(window, cx);
                this.diff.update(cx, |s, cx| {
                    s.set_mode(
                        if this.preferences.diff_layout == "split" {
                            DiffMode::Split
                        } else {
                            DiffMode::Unified
                        },
                        cx,
                    )
                });
                this.refresh(window, cx);
            },
        );
    }
    fn toggle_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let split = self.preferences.diff_layout != "split";
        self.preferences.diff_layout = if split { "split" } else { "unified" }.into();
        self.diff.update(cx, |s, cx| {
            s.set_mode(
                if split {
                    DiffMode::Split
                } else {
                    DiffMode::Unified
                },
                cx,
            )
        });
        let core = self.core.clone();
        let layout = self.preferences.diff_layout.clone();
        let gate = self.gate.clone();
        self.task(
            async move {
                let _guard = gate.lock().await;
                core.store.update(|d| {
                    d.preferences.diff_layout = layout;
                    Ok(())
                })
            },
            window,
            cx,
            |_, (), _, _| {},
        );
    }
    fn persist_widths(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let core = self.core.clone();
        let gate = self.gate.clone();
        let widths = self.preferences.sidebar_widths.clone();
        self.task(
            async move {
                let _guard = gate.lock().await;
                core.store.update(|d| {
                    d.preferences.sidebar_widths = widths;
                    Ok(())
                })
            },
            window,
            cx,
            |_, (), _, _| {},
        );
    }
    fn set_sidebar_width(
        &mut self,
        index: usize,
        width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let total = f32::from(window.viewport_size().width);
        let other = if index == 0 {
            if self.feedback_visible {
                self.preferences.sidebar_widths.feedback as f32
            } else {
                0.
            }
        } else {
            self.preferences.sidebar_widths.files as f32
        };
        let min = if index == 0 { 180. } else { 260. };
        let max: f32 = if index == 0 { 600. } else { 640. };
        let width = width.clamp(min, max.min(total - other - 320. - 8.).max(min)) as u32;
        if index == 0 {
            self.preferences.sidebar_widths.files = width;
        } else {
            self.preferences.sidebar_widths.feedback = width;
        }
        cx.notify();
    }
    fn splitter(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        div().id(("native-splitter",index)).w(px(4.)).h_full().flex_shrink_0().cursor_col_resize()
            .track_focus(&self.resize_focus[index]).tab_index(0)
            .tooltip(|window,cx|tooltip::Tooltip::new("Resize sidebar: arrows, Shift+arrows, Home/End; double-click resets; Escape cancels drag").build(window,cx))
            .on_mouse_down(MouseButton::Left,cx.listener(move|this,event:&MouseDownEvent,window,cx|{
                this.resize_focus[index].focus(window,cx);
                if event.click_count==2 {
                    this.resize_drag=None;this.set_sidebar_width(index,if index==0 {244.}else{310.},window,cx);this.persist_widths(window,cx);
                }else{this.resize_drag=Some((index,f32::from(event.position.x),this.preferences.sidebar_widths.clone()));}
                cx.stop_propagation();
            }))
            .on_key_down(cx.listener(move|this,event:&KeyDownEvent,window,cx|{
                let current=if index==0 {this.preferences.sidebar_widths.files}else{this.preferences.sidebar_widths.feedback} as f32;
                let step=if event.keystroke.modifiers.shift {40.}else{10.};
                let next=match event.keystroke.key.as_str(){"left"=>Some(current+if index==0 {-step}else{step}),"right"=>Some(current+if index==0 {step}else{-step}),"home"=>Some(if index==0 {180.}else{260.}),"end"=>Some(if index==0 {600.}else{640.}),_=>None};
                if let Some(width)=next {this.set_sidebar_width(index,width,window,cx);this.persist_widths(window,cx);cx.stop_propagation();}
            }))
    }
    fn navigation_side(&mut self, side: DiffSide, cx: &mut Context<Self>) {
        if self.navigation_file.is_some() {
            return;
        }
        if let Some(anchor) = &self.anchor {
            if let Some(mapped) = self.diff.read(cx).counterpart_position(anchor, side) {
                self.anchor = Some(mapped);
                self.notice = format!("Navigation version: {}", crate::workspace::side(side));
            } else {
                self.notice = "This changed line exists only on its selected side".into();
            }
        }
        cx.notify();
    }
    fn reveal_navigation_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(file), Some(target)) = (&self.navigation_file, &self.pending_location) else {
            return;
        };
        let position = DiffLinePosition::new(
            file.path.clone(),
            DiffSide::Modified,
            target.start.line as usize + 1,
        );
        let bytes = file
            .content
            .as_deref()
            .and_then(|s| Some(source_byte(s, target.start)?..source_byte(s, target.end)?));
        self.diff.update(cx, |s, cx| {
            s.scroll_to_line(position.clone(), cx);
            if let Some(bytes) = bytes {
                s.select_source_bytes(&file.path, DiffSide::Modified, bytes, cx);
            }
        });
        self.anchor = Some(position);
        self.diff.read(cx).focus_handle(cx).focus(window, cx);
        self.column.update(cx, |s, cx| {
            s.set_value(target.start.character.to_string(), window, cx)
        });
    }
    fn navigate(&mut self, operation: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_navigation();
        self.error = None;
        let (Some(snapshot), Some(anchor)) = (&self.snapshot, &self.anchor) else {
            self.error = Some("Select a source line first".into());
            return;
        };
        let column = self.column.read(cx).value().parse::<u32>().unwrap_or(0);
        let epoch = self.epoch.load(Ordering::SeqCst);
        let generation = self.file_epoch;
        let request_id = format!(
            "native-{epoch}-{generation}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let request = NavigationRequest {
            request_id: request_id.clone(),
            snapshot_id: snapshot.id.clone(),
            side: self
                .navigation_file
                .as_ref()
                .map(|f| f.side.clone())
                .unwrap_or_else(|| side(anchor.side()).into()),
            path: Some(
                if self.navigation_file.is_none() && anchor.side() == DiffSide::Original {
                    self.content
                        .as_ref()
                        .and_then(|f| f.old_path.clone())
                        .unwrap_or_else(|| anchor.path().to_string())
                } else {
                    anchor.path().to_string()
                },
            ),
            document_id: self.navigation_file.as_ref().map(|f| f.document_id.clone()),
            position: Position {
                line: anchor.line() as u32 - 1,
                character: column,
            },
            operation: operation.into(),
        };
        self.navigation_request = Some(request_id.clone());
        self.notice = "Resolving symbol…".into();
        let core = self.core.clone();
        let auto_open = operation != "references";
        self.task(
            async move {
                core.navigate_symbol(request)
                    .await
                    .map_err(|e| format!("{}: {}", e.code, e.message))
            },
            window,
            cx,
            move |this, result, window, cx| {
                if this.epoch.load(Ordering::SeqCst) == epoch
                    && this.file_epoch == generation
                    && this.navigation_request.as_ref() == Some(&request_id)
                {
                    this.navigation_request = None;
                    this.notice = format!("{} results", result.locations.len());
                    let single = result.locations.len() == 1;
                    this.navigation = Some(result);
                    if single && auto_open {
                        this.open_location(0, window, cx);
                    }
                }
            },
        );
    }
    fn open_location(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.remember(cx);
        let (Some(snapshot), Some(result)) = (&self.snapshot, self.navigation.clone()) else {
            return;
        };
        let Some(location) = result.locations.get(index) else {
            return;
        };
        let id = snapshot.id.clone();
        let side = result.side.clone();
        let document = location.document_id.clone();
        let target = location.range.clone();
        let line = target.start.line as usize + 1;
        let core = self.core.clone();
        let width = self.preferences.tab_width;
        let whitespace = self.preferences.show_whitespace;
        self.push_navigation_history(cx);
        self.pending_location = Some(target.clone());
        self.file_epoch += 1;
        let generation = self.file_epoch;
        self.loading = true;
        self.diff.update(cx, |s, cx| s.set_files([], cx));
        self.task(
            async move {
                navigation_models(core,id,side,document,false,width,whitespace).await
            },
            window,
            cx,
            move |this, (file,models), window, cx| {
                if this.file_epoch != generation {
                    return;
                }
                this.loading = false;
                if !models.is_empty() {
                    let position=DiffLinePosition::new(file.path.clone(),DiffSide::Modified,line);
                    this.diff.update(cx,|s,cx|{s.set_files(models,cx);s.scroll_to_line(position.clone(),cx);});
                    this.anchor=Some(position);
                    this.column.update(cx,|s,cx|s.set_value(target.start.character.to_string(),window,cx));
                }
                this.hits.clear();
                this.notice = file.label.clone();
                this.navigation_file = Some(file);
                this.reveal_navigation_target(window,cx);
            },
        );
    }
    fn load_navigation_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.navigation_file.clone() else {
            return;
        };
        let core = self.core.clone();
        let width = self.preferences.tab_width;
        let whitespace = self.preferences.show_whitespace;
        let generation = self.file_epoch;
        self.task(
            async move {
                navigation_models(
                    core,
                    file.snapshot_id,
                    file.side,
                    file.document_id,
                    true,
                    width,
                    whitespace,
                )
                .await
            },
            window,
            cx,
            move |this, (file, models), window, cx| {
                if this.file_epoch != generation {
                    return;
                }
                this.diff.update(cx, |s, cx| s.set_files(models, cx));
                if let Some(target) = &this.pending_location {
                    let position = DiffLinePosition::new(
                        file.path.clone(),
                        DiffSide::Modified,
                        target.start.line as usize + 1,
                    );
                    this.diff
                        .update(cx, |s, cx| s.scroll_to_line(position.clone(), cx));
                    this.anchor = Some(position);
                    this.column.update(cx, |s, cx| {
                        s.set_value(target.start.character.to_string(), window, cx)
                    });
                }
                this.navigation_file = Some(file);
                this.reveal_navigation_target(window, cx);
            },
        );
    }
    fn activate_loaded(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.content.as_ref().is_some_and(|c| c.path == path) {
            return;
        }
        let Some(content) = self.loaded_files.get(path).cloned() else {
            return;
        };
        self.remember(cx);
        self.cancel_navigation();
        self.file_epoch += 1;
        self.reveal_tree(path, window, cx);
        self.content = Some(content);
        self.hits.clear();
        self.search_fresh = true;
        self.anchor = None;
        self.restore_comment(self.saved_comment_view().unwrap_or_default(), window, cx);
        self.remember(cx);
        cx.notify();
    }
    fn set_file_mode(&mut self, all: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.preferences.all_files = all;
        self.pending_all_files = false;
        self.remember(cx);
        // Selecting the already active mode must not replace the Diff/history
        // or launch a second batch read while the user starts Find/QuickOpen.
        if all == self.all_files && !self.loading && self.navigation_file.is_none() {
            return;
        }
        if all {
            self.load_all(window, cx);
        } else if let Some(id) = self.content.as_ref().map(|f| f.file_id.clone()) {
            self.select(id, true, window, cx);
        }
    }
    fn load_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        self.remember(cx);
        self.cancel_navigation();
        self.file_epoch += 1;
        let generation = self.file_epoch;
        let epoch = self.epoch.load(Ordering::SeqCst);
        let core = self.core.clone();
        let whitespace = self.preferences.show_whitespace;
        self.loading = true;
        self.task(async move {
            let mut loaded=BTreeMap::new();let mut parsed=Vec::new();let mut bytes=0usize;let mut skipped=0;
            for entry in snapshot.files {
                if entry.deferred {skipped+=1;continue;}
                let content=core.read_file(&snapshot.id,&entry.id,false).await?;
                bytes+=content.old_content.as_ref().map_or(0,String::len)+content.new_content.as_ref().map_or(0,String::len);
                if bytes>32*1024*1024 {skipped+=1;continue;}
                let code=snapshot.comparison.view=="code";
                let (content,models)=tokio::task::spawn_blocking(move ||{let models=files(&content,code)?.into_iter().map(|f|f.with_display_options(content.tab_width as usize,whitespace)).collect::<Vec<_>>();Ok::<_,String>((content,models))}).await.map_err(|e|e.to_string())??;
                parsed.extend(models);loaded.insert(content.path.clone(),Arc::new(content));
            }
            Ok((loaded,parsed,skipped))
        },window,cx,move |this,(loaded,parsed,skipped),window,cx|{
            if this.epoch.load(Ordering::SeqCst)!=epoch||this.file_epoch!=generation{return;}
            this.loading=false;this.all_files=true;this.loaded_files=loaded;this.diff.update(cx,|s,cx|s.set_files(parsed,cx));
            if !this.content.as_ref().is_some_and(|f|this.loaded_files.contains_key(&f.path)){if let Some(path)=this.loaded_files.keys().next().cloned(){this.activate_loaded(&path,window,cx);}}
            if let Some(path) = this.content.as_ref().map(|f| f.path.clone()) { this.diff.update(cx,|s,cx|s.scroll_to_file(&path,cx)); }
            this.remember(cx);
            this.notice=format!("{} files loaded; {} deferred or outside the 32 MiB source budget. Select skipped files individually.",this.loaded_files.len(),skipped);
        });
    }
    fn remove_repository(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.remember(cx);
        let core = self.core.clone();
        let gate = self.gate.clone();
        let removed = path.clone();
        self.task(
            async move {
                let _guard = gate.lock().await;
                core.store.update(|d| {
                    d.preferences.recent_repositories.retain(|p| p != &path);
                    if d.preferences.last_repository.as_ref() == Some(&path) {
                        d.preferences.last_repository = None;
                    }
                    Ok(())
                })?;
                Ok(core.store.read()?.preferences)
            },
            window,
            cx,
            move |this, preferences, _, cx| {
                this.preferences = preferences;
                if this
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.repository.root == removed)
                {
                    this.epoch.fetch_add(1, Ordering::SeqCst);
                    this.cancel_navigation();
                    this.snapshot = None;
                    this.content = None;
                    this.queue = ReviewQueue::default();
                    this.diff.update(cx, |s, cx| s.set_files([], cx));
                }
            },
        );
    }
    fn relocate_repository(
        &mut self,
        previous: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.remember(cx);
        let path = self.path.read(cx).value().to_string();
        let core = self.core.clone();
        let gate = self.gate.clone();
        self.task(
            async move {
                let _guard = gate.lock().await;
                core.relocate_repository(&previous, &path)
                    .await
                    .map(|s| s.repository.root)
            },
            window,
            cx,
            |this, path, window, cx| this.open(path, None, window, cx),
        );
    }
    fn import_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import self-contained VS Code theme".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let response = receiver.await;
            if let Err(error) = &response {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("Theme import dialog unavailable: {error}"));
                    cx.notify();
                });
            }
            if let Ok(Err(error)) = &response {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("Theme import dialog: {error}"));
                    cx.notify();
                });
            }
            if let Ok(Ok(Some(paths))) = response {
                if let Some(path) = paths.first() {
                    let path = path.to_string_lossy().into_owned();
                    let _ = this.update_in(cx, |this, window, cx| {
                        let core = this.core.clone();
                        this.task(
                            async move { core.import_theme(&path) },
                            window,
                            cx,
                            |this, theme, _, _| {
                                this.themes.retain(|t| t.id != theme.id);
                                this.themes.push(theme);
                                this.notice = "Theme imported; choose it below".into();
                            },
                        );
                    });
                }
            }
        })
        .detach();
    }
    fn remove_theme(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let core = self.core.clone();
        self.task(
            async move {
                core.remove_theme(&id)?;
                Ok((id, core.store.read()?.preferences))
            },
            window,
            cx,
            |this, (id, preferences), window, cx| {
                this.themes.retain(|t| t.id != id);
                this.preferences = preferences;
                if let Some(form) = &mut this.settings_form {
                    if form.draft.appearance.light_theme == id {
                        form.draft.appearance.light_theme =
                            this.preferences.appearance.light_theme.clone();
                    }
                    if form.draft.appearance.dark_theme == id {
                        form.draft.appearance.dark_theme =
                            this.preferences.appearance.dark_theme.clone();
                    }
                }
                this.apply_theme(window, cx);
            },
        );
    }
}

// Bounds are measured in the same window coordinates as pointer events. Capture
// dismisses even if a tree/input child consumes the event; the trigger keeps its
// ordinary toggle behavior, and panel clicks keep selecting commits.
fn history_region(trigger: bool, cx: &Context<Workspace>) -> AnyElement {
    let entity = cx.entity();
    let mouse_entity = entity.clone();
    canvas(
        move |bounds, _, cx| {
            entity.update(cx, |this, _| {
                if trigger {
                    this.history_trigger_bounds = Some(bounds);
                } else {
                    this.history_panel_bounds = Some(bounds);
                }
            });
        },
        move |_, _, window, _| {
            if trigger {
                return;
            }
            // A descendant's opaque hitbox can exclude the root div from
            // hover-based capture listeners. Outside dismissal is a window
            // capture listener, with containment against actual painted bounds.
            let entity = mouse_entity.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                    return;
                }
                entity.update(cx, |this, cx| {
                    if this.history_visible
                        && !this
                            .history_panel_bounds
                            .is_some_and(|b| b.contains(&event.position))
                        && !this
                            .history_trigger_bounds
                            .is_some_and(|b| b.contains(&event.position))
                    {
                        this.history_visible = false;
                        cx.notify();
                    }
                });
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Retain a usable center when restoring widths on a smaller display.
        let available = (f32::from(window.viewport_size().width) - 328.).max(440.);
        let files_max = if self.feedback_visible {
            (available - 260.).min(600.)
        } else {
            available.min(600.)
        };
        self.preferences.sidebar_widths.files =
            (self.preferences.sidebar_widths.files as f32).clamp(180., files_max.max(180.)) as u32;
        if self.feedback_visible {
            self.preferences.sidebar_widths.feedback =
                (self.preferences.sidebar_widths.feedback as f32).clamp(
                    260.,
                    (available - self.preferences.sidebar_widths.files as f32)
                        .min(640.)
                        .max(260.),
                ) as u32;
        }
        let filter = self.filter.read(cx).value().to_lowercase();
        let entries: Vec<_> = self
            .snapshot
            .as_ref()
            .map(|s| {
                s.files
                    .iter()
                    .filter(|f| f.path.to_lowercase().contains(&filter))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let quick_selected = if self.filter.read(cx).focus_handle(cx).is_focused(window) {
            entries.get(self.quick_index).map(|f| f.path.clone())
        } else {
            None
        };
        let rows = self.tree_rows(cx);
        let selected = quick_selected
            .as_ref()
            .map(|p| format!("file:{p}"))
            .or_else(|| self.tree_cursor.clone());
        if self.quick_scroll_pending || self.tree_reveal_pending {
            if let Some(ix) = rows
                .iter()
                .position(|r| Some(&r.key()) == selected.as_ref())
            {
                self.tree_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
            }
            self.quick_scroll_pending = false;
            self.tree_reveal_pending = false;
        }
        let queue = self.queue.clone();
        let weak = cx.entity().downgrade();
        let collapsed = self.collapsed.clone();
        let tree_query = filter.clone();
        let max_indent =
            ((self.preferences.sidebar_widths.files.saturating_sub(110)) / 14) as usize;
        let tree = uniform_list("file-tree", rows.len(), move |range, _, cx| {
            range
                .map(|ix| {
                    let row = &rows[ix];
                    let path = row.path.clone();
                    let hint = path.clone();
                    let weak = weak.clone();
                    let expanded = !collapsed.contains(&path);
                    let folder = row.file.is_none();
                    let checked = queue.reviewed.contains_key(&path);
                    let commented = queue.comments.iter().any(|c| c.path == path);
                    let status = row.file.as_ref().map(|f| f.status.as_str()).unwrap_or("");
                    let color = match status {
                        "added" | "untracked" => cx.theme().success,
                        "deleted" | "conflict" => cx.theme().danger,
                        _ => cx.theme().muted_foreground,
                    };
                    list::ListItem::new(SharedString::from(format!("tree-{}", row.key())))
                        .accessibility_label(path.clone())
                        .h(px(32.))
                        .text_sm()
                        .w_full()
                        .min_w_0()
                        .gap_1()
                        .pl(px(8. + row.depth.min(max_indent) as f32 * 14.))
                        .pr_2()
                        .border_l_2()
                        .border_color(if selected.as_ref() == Some(&row.key()) {
                            cx.theme().primary
                        } else {
                            transparent_black()
                        })
                        .selected(selected.as_ref() == Some(&row.key()))
                        .tooltip(move |w, c| tooltip::Tooltip::new(hint.clone()).build(w, c))
                        .child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .gap_1()
                                .child(div().w(px(12.)).flex_shrink_0().when(folder, |d| {
                                    d.child(
                                        Icon::new(if expanded {
                                            IconName::ChevronDown
                                        } else {
                                            IconName::ChevronRight
                                        })
                                        .size(px(12.)),
                                    )
                                }))
                                .child(
                                    Icon::new(if folder {
                                        if expanded {
                                            gpui_kit::assets::IconName::FolderOpen
                                        } else {
                                            gpui_kit::assets::IconName::Folder
                                        }
                                    } else {
                                        gpui_kit::assets::IconName::FileCode
                                    })
                                    .size(px(14.))
                                    .flex_shrink_0(),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(
                                            StyledText::new(format!(
                                                "{}{}",
                                                if row.depth > max_indent { "…/" } else { "" },
                                                row.label
                                            ))
                                            .with_highlights(
                                                file_tree::matches(&row.label, &tree_query)
                                                    .into_iter()
                                                    .map(|range| {
                                                        (
                                                            range,
                                                            HighlightStyle {
                                                                background_color: Some(
                                                                    cx.theme().selection,
                                                                ),
                                                                font_weight: Some(FontWeight::BOLD),
                                                                ..Default::default()
                                                            },
                                                        )
                                                    }),
                                            ),
                                        ),
                                )
                                .when(commented, |item| {
                                    item.child(
                                        Icon::new(gpui_kit::assets::IconName::MessageSquare)
                                            .size(px(12.))
                                            .flex_shrink_0(),
                                    )
                                })
                                .when(checked, |item| {
                                    item.child(
                                        Icon::new(IconName::Check)
                                            .size(px(13.))
                                            .text_color(cx.theme().success)
                                            .flex_shrink_0(),
                                    )
                                })
                                .when(!checked && !folder, |item| {
                                    item.child(
                                        div()
                                            .text_xs()
                                            .text_color(color)
                                            .flex_shrink_0()
                                            .child(file_tree::status(status)),
                                    )
                                }),
                        )
                        .on_click(move |_, window, cx| {
                            let _ = weak
                                .update(cx, |this, cx| this.tree_click(&path, folder, window, cx));
                        })
                        .into_any_element()
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.tree_scroll)
        .flex_1()
        .min_h_0();
        let tree = v_flex()
            .id("repository-file-tree")
            .key_context("PeerFileTree")
            .track_focus(&self.tree_focus)
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(tree)
            .vertical_scrollbar(&self.tree_scroll)
            .on_action(cx.listener(|this, _: &TreeUp, w, c| this.tree_key("up", w, c)))
            .on_action(cx.listener(|this, _: &TreeDown, w, c| this.tree_key("down", w, c)))
            .on_action(cx.listener(|this, _: &TreeLeft, w, c| this.tree_key("left", w, c)))
            .on_action(cx.listener(|this, _: &TreeRight, w, c| this.tree_key("right", w, c)))
            .on_action(cx.listener(|this, _: &TreeConfirm, w, c| this.tree_key("enter", w, c)))
            .on_action(cx.listener(|this, _: &TreeFirst, w, c| this.tree_key("home", w, c)))
            .on_action(cx.listener(|this, _: &TreeLast, w, c| this.tree_key("end", w, c)));
        let queue = self.queue.clone();
        let mut feedback = v_flex().gap_2().p_2();
        for comment in queue.comments {
            let delete = comment.id.clone();
            let edit = comment.clone();
            let jump = comment.clone();
            let position = comment
                .anchor
                .line
                .map(|line| {
                    format!(
                        " {}:{}",
                        comment.anchor.side.as_deref().unwrap_or("new"),
                        line
                    )
                })
                .unwrap_or_default();
            feedback = feedback.child(
                v_flex()
                    .gap_1()
                    .p_2()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .id(SharedString::from(format!("jump-{}", comment.id)))
                            .cursor_pointer()
                            .whitespace_normal()
                            .child(comment.path.clone())
                            .on_click(cx.listener(move |this, _, w, c| {
                                this.jump_comment(jump.clone(), w, c)
                            })),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .child(if position.is_empty() {
                                "File comment".to_string()
                            } else {
                                position.trim().to_string()
                            })
                            .when(comment.outdated, |row| {
                                row.child(
                                    div()
                                        .px_1()
                                        .border_1()
                                        .border_color(cx.theme().warning)
                                        .child("Outdated"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "You · {}",
                                comment
                                    .created_at
                                    .split('T')
                                    .next()
                                    .unwrap_or(&comment.created_at)
                            )),
                    )
                    .when(self.editing.as_ref() != Some(&comment.id), |card| {
                        card.child(comment.body.clone())
                    })
                    .when(self.editing.as_ref() == Some(&comment.id), |card| {
                        card.child(div().text_xs().child("Edit comment"))
                            .child(Textarea::new(&self.edit_comment).h(px(100.)))
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new("save-comment-edit")
                                            .small()
                                            .label("Save edit")
                                            .disabled(self.feedback_pending)
                                            .on_click(
                                                cx.listener(|this, _, w, c| {
                                                    this.save_comment(w, c)
                                                }),
                                            ),
                                    )
                                    .child(
                                        Button::new("cancel-card-edit")
                                            .small()
                                            .label("Cancel edit")
                                            .on_click(
                                                cx.listener(|this, _, w, c| this.cancel_edit(w, c)),
                                            ),
                                    ),
                            )
                    })
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new(SharedString::from(format!("edit-{}", edit.id)))
                                    .small()
                                    .label("Edit")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if this.editing.is_none() {
                                            this.pending_new =
                                                Some(Box::new(this.comment_view(cx)));
                                        }
                                        this.editing = Some(edit.id.clone());
                                        this.edit_comment.update(cx, |s, cx| {
                                            s.set_value(edit.body.clone(), window, cx)
                                        });
                                        this.edit_comment.update(cx, |s, cx| s.focus(window, cx));
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("delete-{delete}")))
                                    .small()
                                    .label("Delete")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.delete_comment(delete.clone(), window, cx)
                                    })),
                            ),
                    )
                    .when_some(comment.anchor.line_text.clone(), |this, text| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("Saved source: {text}")),
                        )
                    }),
            );
        }
        let scope = if let Some(anchor) = &self.comment_anchor {
            format!("Comment on {} line {}", side(anchor.side()), anchor.line())
        } else {
            "File comment".into()
        };
        let mut annotations = Vec::new();
        let bodies: BTreeMap<_, _> = self
            .queue
            .comments
            .iter()
            .filter(|c| !c.outdated)
            .map(|c| (c.id.clone(), c.body.clone()))
            .collect();
        if self.navigation_file.is_none() {
            for comment in &self.queue.comments {
                if !comment.outdated {
                    if let Some(line) = comment.anchor.line {
                        annotations.push(DiffAnnotation::line(
                            SharedString::from(comment.id.clone()),
                            DiffLinePosition::new(
                                comment.path.clone(),
                                diff_side(comment.anchor.side.as_deref().unwrap_or("new")),
                                line as usize,
                            ),
                        ));
                    } else {
                        annotations.push(DiffAnnotation::file(
                            SharedString::from(comment.id.clone()),
                            comment.path.clone(),
                        ));
                    }
                }
            }
        }
        let weak = cx.entity().downgrade();
        let click_weak = weak.clone();
        let header_weak = weak.clone();
        let mut header_files = self.loaded_files.clone();
        if let Some(content) = &self.content {
            header_files.insert(content.path.clone(), content.clone());
        }
        let header_reviewed = self.queue.reviewed.clone();
        let header_disabled = self.loading || self.navigation_file.is_some();

        let search_ranges = self
            .content
            .as_ref()
            .map(|f| {
                self.hits
                    .iter()
                    .map(|h| {
                        (
                            DiffLinePosition::new(
                                self.navigation_file
                                    .as_ref()
                                    .map(|n| n.path.clone())
                                    .unwrap_or_else(|| f.path.clone()),
                                diff_side(h.side),
                                h.line as usize,
                            ),
                            h.byte..h.end_byte,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let viewer = Diff::new(&self.diff)
            .size_full()
            .render_header_suffix(move |file, _, _| {
                let content = header_files.get(file.path().as_str()).cloned();
                let checked = content
                    .as_ref()
                    .is_some_and(|f| header_reviewed.get(&f.path) == Some(&f.fingerprint));
                let disabled = header_disabled || content.is_none();
                let review_content = content.clone();
                let review_weak = header_weak.clone();
                let comment_weak = header_weak.clone();
                let path = file.path().to_string();
                h_flex()
                    .gap_2()
                    .flex_shrink_0()
                    .child(
                        checkbox::Checkbox::new("file-reviewed")
                            .label("Reviewed")
                            .checked(checked)
                            .disabled(disabled)
                            .on_click(move |checked, window, cx| {
                                if let Some(content) = &review_content {
                                    let _ = review_weak.update(cx, |this, cx| {
                                        this.diff.update(cx, |s, _| s.preserve_viewport_identity());
                                        this.activate_loaded(&content.path, window, cx);
                                        this.reviewed_file(content.clone(), *checked, window, cx);
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("comment-on-file")
                            .small()
                            .label("Comment on file")
                            .disabled(disabled)
                            .on_click(move |_, window, cx| {
                                let _ = comment_weak.update(cx, |this, cx| {
                                    this.diff.update(cx, |s, _| s.preserve_viewport_identity());
                                    this.activate_loaded(&path, window, cx);
                                    this.feedback_visible = true;
                                    this.begin_comment(None, window, cx);
                                });
                            }),
                    )
            })
            .sticky_header_path(
                self.navigation_file
                    .as_ref()
                    .map(|f| SharedString::from(f.path.clone()))
                    .or_else(|| {
                        self.content
                            .as_ref()
                            .map(|f| SharedString::from(f.path.clone()))
                    }),
            )
            .annotations(annotations)
            .search_highlights(search_ranges)
            .render_annotation(move |annotation, _, _| {
                let body = bodies
                    .iter()
                    .find(|(id, _)| {
                        annotation.id() == &ElementId::Name(SharedString::from((*id).clone()))
                    })
                    .map(|(_, body)| body.clone())
                    .unwrap_or_default();
                div().p_2().child(body)
            })
            .on_add_annotation(move |range, window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if this.navigation_file.is_some() {
                        return;
                    }
                    this.feedback_visible = true;
                    this.diff.update(cx, |s, _| s.preserve_viewport_identity());
                    this.activate_loaded(range.path(), window, cx);
                    this.begin_comment(
                        Some(DiffLinePosition::new(
                            range.path().clone(),
                            range.side(),
                            range.start(),
                        )),
                        window,
                        cx,
                    );
                    cx.notify();
                });
            })
            .on_line_click(move |position, event, window, cx| {
                let _ = click_weak.update(cx, |this, cx| {
                    this.diff.update(cx, |s, _| s.preserve_viewport_identity());
                    this.activate_loaded(position.path(), window, cx);
                    this.anchor = Some(position.clone());
                    if let ClickEvent::Mouse(mouse) = event {
                        if let Some((position, byte)) =
                            this.diff.read(cx).position_at(mouse.up.position)
                        {
                            let source = if let Some(file) = &this.navigation_file {
                                file.content.as_deref()
                            } else {
                                this.content.as_ref().and_then(|f| {
                                    if position.side() == DiffSide::Original {
                                        f.old_content.as_deref()
                                    } else {
                                        f.new_content.as_deref()
                                    }
                                })
                            };
                            if let Some(line) = source
                                .and_then(|s| s.split_inclusive('\n').nth(position.line() - 1))
                            {
                                if let Ok(column) = presentation::utf16_column(line, byte) {
                                    this.column.update(cx, |s, cx| {
                                        s.set_value(column.to_string(), window, cx)
                                    });
                                }
                            }
                        }
                    }
                    if let ClickEvent::Mouse(mouse) = event {
                        if mouse.up.modifiers.platform || mouse.up.modifiers.control {
                            this.navigate("auto", window, cx);
                        }
                    }
                    cx.notify();
                });
            });
        let viewer = div()
            .id("diff-context-menu")
            .size_full()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if let Some((position, byte)) = this.diff.read(cx).position_at(event.position) {
                        this.diff.update(cx, |s, _| s.preserve_viewport_identity());
                        this.activate_loaded(position.path(), window, cx);
                        let source = this
                            .navigation_file
                            .as_ref()
                            .and_then(|f| f.content.as_deref())
                            .or_else(|| {
                                this.content.as_ref().and_then(|f| {
                                    if position.side() == DiffSide::Original {
                                        f.old_content.as_deref()
                                    } else {
                                        f.new_content.as_deref()
                                    }
                                })
                            });
                        let column = source
                            .and_then(|s| s.split_inclusive('\n').nth(position.line() - 1))
                            .and_then(|line| presentation::utf16_column(line, byte).ok());
                        this.anchor = Some(position);
                        if let Some(column) = column {
                            this.column
                                .update(cx, |s, cx| s.set_value(column.to_string(), window, cx));
                        }
                    }
                }),
            )
            .child(viewer)
            .context_menu(|menu, _, _| {
                menu.menu("Go to definition", Box::new(Definition))
                    .menu("Find references", Box::new(References))
            });
        let mut center = v_flex().flex_1().min_w_0().min_h_0();
        if self.settings_visible {
            center = center.child(self.settings_panel(window, cx));
        } else {
            center =
                center.child(
                    h_flex()
                        .gap_1()
                        .p_2()
                        .child(Input::new(&self.search).w(px(240.)))
                        .child(
                            Button::new("find-prev")
                                .small()
                                .label("Previous")
                                .on_click(cx.listener(|this, _, w, cx| this.find(-1, w, cx))),
                        )
                        .child(
                            Button::new("find-next")
                                .small()
                                .label("Find / Next")
                                .on_click(cx.listener(|this, _, w, cx| this.find(1, w, cx))),
                        )
                        .child(Input::new(&self.line).w(px(70.)))
                        .child(
                            Button::new("jump")
                                .small()
                                .label("Go")
                                .on_click(cx.listener(|this, _, _, cx| this.jump(cx))),
                        )
                        .child(
                            Button::new("layout")
                                .small()
                                .label(self.preferences.diff_layout.clone())
                                .on_click(cx.listener(|this, _, w, c| this.toggle_layout(w, c))),
                        )
                        .child(
                            Button::new("expand")
                                .small()
                                .label("Expand")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.diff.update(cx, |s, cx| s.expand_unchanged(cx))
                                })),
                        )
                        .child(Button::new("collapse").small().label("Fold").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.diff.update(cx, |s, cx| s.collapse_unchanged(cx))
                            }),
                        )),
                );
            if let Some(content) = &self.content {
                if content.deferred {
                    let id = content.file_id.clone();
                    center = center
                        .child(div().p_4().child(format!(
                            "{} bytes — explicit loading required",
                            content.size
                        )))
                        .child(Button::new("load-large").label("Load large file").on_click(
                            cx.listener(move |this, _, w, c| this.select(id.clone(), true, w, c)),
                        ));
                } else if content.kind != "text" {
                    center = center.child(
                        div()
                            .p_4()
                            .child(format!("{}: {}", content.kind, content.metadata)),
                    );
                }
            }
            if self.navigation_file.as_ref().is_some_and(|f| f.deferred) {
                center = center.child(
                    Button::new("load-navigation")
                        .label("Load large navigation source")
                        .on_click(cx.listener(|this, _, w, c| this.load_navigation_source(w, c))),
                );
            }
            center = center.child(div().flex_1().min_h_0().child(viewer));
            center = center.child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .child(Input::new(&self.column).w(px(200.)))
                    .child(
                        Button::new("definition")
                            .small()
                            .label("Definition F12")
                            .on_click(cx.listener(|this, _, w, c| this.navigate("auto", w, c))),
                    )
                    .child(
                        Button::new("references")
                            .small()
                            .label("References ⇧F12")
                            .on_click(
                                cx.listener(|this, _, w, c| this.navigate("references", w, c)),
                            ),
                    )
                    .child(
                        Button::new("cancel-navigation")
                            .small()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cancel_navigation();
                                cx.notify();
                            })),
                    ),
            );
            center = center.child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("navigation-old")
                            .small()
                            .label("Original A")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigation_side(DiffSide::Original, cx)
                            })),
                    )
                    .child(
                        Button::new("navigation-new")
                            .small()
                            .label("Modified B")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigation_side(DiffSide::Modified, cx)
                            })),
                    ),
            );
            if let Some(result) = &self.navigation {
                let mut results = v_flex().gap_1();
                for (index, location) in result.locations.iter().enumerate() {
                    results = results.child(
                        Button::new(("location", index))
                            .ghost()
                            .label(format!(
                                "{}:{} {} — {}",
                                location.path,
                                location.range.start.line + 1,
                                location.label,
                                location.preview
                            ))
                            .on_click(
                                cx.listener(move |this, _, w, c| this.open_location(index, w, c)),
                            ),
                    );
                }
                center = center.child(
                    div()
                        .id("locations")
                        .max_h(px(180.))
                        .overflow_y_scroll()
                        .child(results),
                );
            }
        }
        let mut sidebar = v_flex()
            .size_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .min_h_0()
            .child(
                h_flex()
                    .h(px(24.))
                    .px_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        if self
                            .snapshot
                            .as_ref()
                            .is_some_and(|s| s.comparison.view == "code")
                        {
                            "REPOSITORY"
                        } else {
                            "CHANGED FILES"
                        },
                    )
                    .child(div().flex_1())
                    .child(format!(
                        "{}",
                        self.snapshot.as_ref().map(|s| s.files.len()).unwrap_or(0)
                    )),
            )
            .child(
                div()
                    .id("quick-open")
                    .key_context("PeerFileFilter")
                    .p_2()
                    .child(
                        Input::new(&self.filter).prefix(Icon::new(IconName::Search).size(px(14.))),
                    ),
            )
            .child(tree)
            .child(
                h_flex()
                    .h(px(24.))
                    .px_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Local repository")
                    .child(div().flex_1())
                    .child("Read only"),
            );
        if self.history_visible {
            let mut commits = v_flex().gap_1();
            for commit in self.commits.clone() {
                commits = commits.child(
                    Button::new(SharedString::from(commit.id.clone()))
                        .ghost()
                        .h_auto()
                        .w_full()
                        .justify_start()
                        .py_2()
                        .accessibility_label(format!(
                            "View commit {}: {}",
                            commit.short_id, commit.subject
                        ))
                        .child(
                            v_flex()
                                .items_start()
                                .gap_1()
                                .child(div().whitespace_normal().child(format!(
                                        "{}{}",
                                        if self
                                            .snapshot
                                            .as_ref()
                                            .and_then(|s| s.commit.as_ref())
                                            .is_some_and(|c| c.id == commit.id)
                                        {
                                            "✓ "
                                        } else {
                                            ""
                                        },
                                        commit.subject
                                    )))
                                .child(
                                    div()
                                        .text_xs()
                                        .whitespace_normal()
                                        .child(format!("{} · {}", commit.short_id, commit.author)),
                                )
                                .child(
                                    div().text_xs().child(
                                        commit
                                            .committed_at
                                            .split('T')
                                            .next()
                                            .unwrap_or(&commit.committed_at)
                                            .to_string(),
                                    ),
                                ),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.reference
                                .update(cx, |s, cx| s.set_value(commit.id.clone(), window, cx));
                            this.compare("commit", window, cx);
                        })),
                );
            }
            sidebar = sidebar.child(
                v_flex()
                    .relative()
                    .child(
                        div()
                            .id("commits")
                            .h(px(240.))
                            .overflow_y_scroll()
                            .child(commits),
                    )
                    .child(
                        Button::new("more")
                            .label("Load older commits")
                            .disabled(!self.more_commits || self.commits_loading)
                            .on_click(cx.listener(|this, _, w, c| this.load_commits(w, c))),
                    )
                    .child(history_region(false, cx)),
            );
        }
        let mut root = v_flex()
            .id("peer")
            .key_context("Peer")
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if let Some((index, start, widths)) = this.resize_drag.clone() {
                    let delta = f32::from(event.position.x) - start;
                    let width = if index == 0 {
                        widths.files as f32 + delta
                    } else {
                        widths.feedback as f32 - delta
                    };
                    this.set_sidebar_width(index, width, window, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if this.resize_drag.take().is_some() {
                        this.persist_widths(window, cx);
                    }
                }),
            )
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.history_visible && event.keystroke.key == "escape" {
                    this.history_visible = false;
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &SaveComment, w, c| this.save_comment(w, c)))
            .track_focus(&self.focus)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .on_action(cx.listener(|this, _: &NextFile, window, cx| {
                if this.quick_visible {
                    let count = this.palette_files(cx).len();
                    if count > 0 {
                        this.quick_index = (this.quick_index + 1) % count;
                    }
                    this.palette_scroll_pending = true;
                    window.refresh();
                    cx.notify();
                    return;
                }
                let filter = this.filter.read(cx).value().to_lowercase();
                let count = this
                    .snapshot
                    .as_ref()
                    .map(|s| {
                        s.files
                            .iter()
                            .filter(|f| f.path.to_lowercase().contains(&filter))
                            .count()
                    })
                    .unwrap_or(0);
                if count > 0 {
                    this.quick_index = (this.quick_index + 1) % count;
                    this.quick_scroll_pending = true;
                    this.notice =
                        format!("Quick open result {} of {}", this.quick_index + 1, count);
                }
                window.refresh();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PreviousFile, window, cx| {
                if this.quick_visible {
                    let count = this.palette_files(cx).len();
                    if count > 0 {
                        this.quick_index = (this.quick_index + count - 1) % count;
                    }
                    this.palette_scroll_pending = true;
                    window.refresh();
                    cx.notify();
                    return;
                }
                this.quick_index = this.quick_index.saturating_sub(1);
                this.quick_scroll_pending = true;
                window.refresh();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Open, w, c| this.choose_folder(w, c)))
            .on_action(cx.listener(|this, _: &Refresh, w, c| this.refresh(w, c)))
            .on_action(cx.listener(|this, _: &QuickOpen, w, c| {
                this.quick_previous_focus = w.focused(c);
                this.quick_visible = true;
                this.quick_index = 0;
                this.palette_scroll_pending = true;
                this.quick_input.update(c, |s, c| {
                    s.set_value("", w, c);
                    s.focus(w, c);
                });
                c.notify();
            }))
            .on_action(
                cx.listener(|this, _: &Find, w, c| this.search.update(c, |s, c| s.focus(w, c))),
            )
            .on_action(
                cx.listener(|this, _: &GoLine, w, c| this.line.update(c, |s, c| s.focus(w, c))),
            )
            .on_action(cx.listener(|this, _: &Back, w, c| this.back(w, c)))
            .on_action(cx.listener(|this, _: &Settings, w, c| this.show_settings(w, c)))
            .on_action(cx.listener(|this, _: &Definition, w, c| this.navigate("auto", w, c)))
            .on_action(cx.listener(|this, _: &References, w, c| this.navigate("references", w, c)))
            .on_action(cx.listener(|this, _: &Cancel, w, c| {
                if let Some((_, _, widths)) = this.resize_drag.take() {
                    this.preferences.sidebar_widths = widths;
                    this.persist_widths(w, c);
                    c.notify();
                    return;
                }
                if this.quick_visible {
                    this.close_palette(w, c);
                    return;
                }
                if this.history_visible {
                    this.history_visible = false;
                    c.notify();
                    return;
                }
                if this.editing.is_some() {
                    this.cancel_edit(w, c);
                    return;
                }
                this.history_visible = false;
                this.branches_visible = false;
                this.cancel_navigation();
                this.settings_visible = false;
                this.focus.focus(w, c);
                this.apply_theme(w, c);
                this.recent_visible = false;
                c.notify();
            }))
            .child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .child(div().font_bold().child("PEER"))
                    .child(Input::new(&self.path).flex_1())
                    .child(
                        Button::new("feedback-toggle")
                            .label("Feedback")
                            .selected(self.feedback_visible)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.feedback_visible = !this.feedback_visible;
                                cx.notify();
                            })),
                    )
                    .child(Button::new("open-path").label("Open").on_click(cx.listener(
                        |this, _, w, c| {
                            this.open(this.path.read(c).value().to_string(), None, w, c)
                        },
                    )))
                    .child(
                        Button::new("browse")
                            .label("Browse…")
                            .on_click(cx.listener(|this, _, w, c| this.choose_folder(w, c))),
                    )
                    .child(Button::new("recent").label("Recent").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.recent_visible = !this.recent_visible;
                            cx.notify();
                        },
                    )))
                    .child(
                        Button::new("refresh")
                            .label("Refresh")
                            .on_click(cx.listener(|this, _, w, c| this.refresh(w, c))),
                    )
                    .child(
                        Button::new("back")
                            .label("Back")
                            .on_click(cx.listener(|this, _, w, c| this.back(w, c))),
                    )
                    .child(
                        Button::new("settings")
                            .label("Settings")
                            .on_click(cx.listener(|this, _, w, c| this.show_settings(w, c))),
                    ),
            );
        if self.recent_visible {
            let mut recent = v_flex().gap_1();
            for (index, path) in self
                .preferences
                .recent_repositories
                .clone()
                .into_iter()
                .enumerate()
            {
                let open = path.clone();
                let relocate = path.clone();
                let remove = path.clone();
                recent = recent.child(
                    h_flex()
                        .gap_2()
                        .child(Button::new(("recent", index)).small().label(path).on_click(
                            cx.listener(move |this, _, w, c| {
                                this.recent_visible = false;
                                this.open(open.clone(), None, w, c);
                            }),
                        ))
                        .child(
                            Button::new(("relocate", index))
                                .small()
                                .label("Move to path above")
                                .on_click(cx.listener(move |this, _, w, c| {
                                    this.relocate_repository(relocate.clone(), w, c)
                                })),
                        )
                        .child(
                            Button::new(("remove-repository", index))
                                .small()
                                .label("Remove from list")
                                .on_click(cx.listener(move |this, _, w, c| {
                                    this.remove_repository(remove.clone(), w, c)
                                })),
                        ),
                );
            }
            root = root.child(
                div()
                    .id("recent-repositories")
                    .max_h(px(240.))
                    .overflow_y_scroll()
                    .child(recent),
            );
        }
        let mut comparisons = h_flex().p_2().gap_1().flex_wrap();
        for (mode, label) in [
            ("code", "Code"),
            ("all", "All changes"),
            ("staged", "Staged"),
            ("unstaged", "Unstaged"),
        ] {
            comparisons = comparisons.child(
                Button::new(mode)
                    .selected(self.snapshot.as_ref().is_some_and(|s| {
                        if mode == "code" {
                            s.comparison.view == "code"
                        } else {
                            s.comparison.view == "changes" && s.comparison.mode == mode
                        }
                    }))
                    .small()
                    .label(label)
                    .on_click(cx.listener(move |this, _, w, c| this.compare(mode, w, c))),
            );
        }
        root = root.child(
            comparisons
                .child(Input::new(&self.reference).w(px(180.)))
                .child(
                    Button::new("branch")
                        .selected(self.snapshot.as_ref().is_some_and(|s| {
                            s.comparison.view == "changes" && s.comparison.mode == "branch"
                        }))
                        .small()
                        .label("Compare branch")
                        .on_click(cx.listener(|this, _, w, c| this.compare("branch", w, c))),
                )
                .child(
                    Button::new("commit")
                        .selected(self.snapshot.as_ref().is_some_and(|s| {
                            s.comparison.view == "changes" && s.comparison.mode == "commit"
                        }))
                        .small()
                        .label("Compare commit")
                        .on_click(cx.listener(|this, _, w, c| this.compare("commit", w, c))),
                )
                .child(
                    Button::new("all-files")
                        .small()
                        .label("All files")
                        .selected(self.all_files)
                        .on_click(cx.listener(|this, _, w, c| this.set_file_mode(true, w, c))),
                )
                .child(
                    Button::new("single-file")
                        .small()
                        .label("Single file")
                        .selected(!self.all_files)
                        .on_click(cx.listener(|this, _, w, c| this.set_file_mode(false, w, c))),
                )
                .child(
                    div()
                        .relative()
                        .child(
                            Button::new("history")
                                .small()
                                .label("Commit history")
                                .on_click(cx.listener(|this, _, w, c| {
                                    this.history_visible = !this.history_visible;
                                    if this.history_visible && this.commits.is_empty() {
                                        this.load_commits(w, c);
                                    }
                                    c.notify();
                                })),
                        )
                        .child(history_region(true, cx)),
                )
                .child(
                    Button::new("resume-changes")
                        .small()
                        .label("Changes")
                        .on_click(cx.listener(|this, _, w, c| this.resume_changes(w, c))),
                )
                .child(
                    Button::new("choose-branch")
                        .small()
                        .label("Branches")
                        .on_click(cx.listener(|this, _, _, c| {
                            this.branches_visible = !this.branches_visible;
                            c.notify();
                        })),
                )
                .child(
                    Button::new("include-untracked")
                        .small()
                        .label("Untracked")
                        .disabled(self.snapshot.as_ref().is_none_or(|s| {
                            ["staged", "commit"].contains(&s.comparison.mode.as_str())
                        }))
                        .selected(
                            self.snapshot
                                .as_ref()
                                .is_some_and(|s| s.comparison.include_untracked),
                        )
                        .on_click(cx.listener(|this, _, w, c| {
                            if let Some(snapshot) = &this.snapshot {
                                let mut comparison = snapshot.comparison.clone();
                                comparison.include_untracked = !comparison.include_untracked;
                                this.open(snapshot.repository.root.clone(), Some(comparison), w, c);
                            }
                        })),
                ),
        );
        if let Some(snapshot) = &self.snapshot {
            let additions: u64 = snapshot.files.iter().map(|f| f.additions).sum();
            let deletions: u64 = snapshot.files.iter().map(|f| f.deletions).sum();
            let mode = if snapshot.comparison.view == "code" {
                "Code"
            } else {
                snapshot.comparison.mode.as_str()
            };
            let mut summary = format!(
                "{} · {} · {} files · +{} −{}",
                mode,
                snapshot.repository.branch,
                snapshot.files.len(),
                additions,
                deletions
            );
            if let Some(commit) = &snapshot.commit {
                summary.push_str(&format!(
                    " · {} · {} · {} · {}",
                    commit.short_id,
                    commit.subject,
                    commit.author,
                    commit
                        .committed_at
                        .split('T')
                        .next()
                        .unwrap_or(&commit.committed_at)
                ));
            } else if let Some(base) = &snapshot.comparison.base_ref {
                summary.push_str(&format!(" · compared with {base}"));
            }
            root = root.child(div().px_2().h(px(24.)).text_xs().child(summary));
        }
        if self.branches_visible {
            let mut branch_rows = h_flex().gap_1().flex_wrap();
            for branch in self.branches.clone() {
                branch_rows = branch_rows.child(
                    Button::new(SharedString::from(format!("choose-branch-{branch}")))
                        .small()
                        .label(branch.clone())
                        .on_click(cx.listener(move |this, _, w, c| {
                            this.reference
                                .update(c, |s, c| s.set_value(branch.clone(), w, c));
                            this.branches_visible = false;
                            this.compare("branch", w, c);
                        })),
                );
            }
            root = root.child(branch_rows);
        }
        if let Some(error) = &self.error {
            root = root.child(
                div()
                    .p_2()
                    .bg(rgb(0x702b36))
                    .text_color(rgb(0xffffff))
                    .child(error.clone()),
            );
        }
        let feedback_panel =
            v_flex()
                .size_full()
                .min_h_0()
                .border_l_1()
                .border_color(cx.theme().border)
                .child(
                    h_flex()
                        .p_2()
                        .gap_1()
                        .child(
                            Button::new("reviewed")
                                .small()
                                .label("Toggle Reviewed")
                                .disabled(
                                    self.loading
                                        || self.content.is_none()
                                        || self.navigation_file.is_some(),
                                )
                                .on_click(cx.listener(|this, _, w, c| this.reviewed(w, c))),
                        )
                        .child(
                            Button::new("copy-export")
                                .small()
                                .label("Copy")
                                .on_click(cx.listener(|this, _, w, c| this.export(false, w, c))),
                        )
                        .child(
                            Button::new("save-export")
                                .small()
                                .label("Export")
                                .on_click(cx.listener(|this, _, w, c| this.export(true, w, c))),
                        ),
                )
                .child(v_flex().p_1().text_xs().whitespace_normal()
                    .child(self.content.as_ref().map(|f| f.path.clone()).unwrap_or_else(|| "Select a file to comment".into()))
                    .child(scope))
                .child(Textarea::new(&self.comment).h(px(110.)))
                .when(!self.comment.read(cx).value().is_empty() && self.editing.is_none() && self.comment_fingerprint.as_ref().zip(self.content.as_ref()).is_some_and(|(fp,f)| fp != &f.fingerprint), |panel| panel.child(div().p_2().child("Draft source changed. Choose File scope or Line scope to confirm a new anchor before saving.")))
                .child(
                    h_flex()
                        .p_2()
                        .gap_1()
                        .child(
                            Button::new("save-comment")
                                .primary()
                                .label("Save comment")
                                .disabled(
                                    self.loading
                                        || self.feedback_pending
                                        || self.content.is_none()
                                        || self.navigation_file.is_some(),
                                )
                                .on_click(cx.listener(|this, _, w, c| this.save_new_comment(w, c))),
                        )
                        .child(Button::new("file-comment").label("File scope").on_click(
                            cx.listener(|this, _, window, cx| {
                                this.begin_comment(None,window,cx);
                            }),
                        )),
                )
                .child(
                    div()
                        .id("feedback")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(feedback),
                )
                .child(if self.clear_queue_request.is_some() {
                    h_flex()
                        .p_2()
                        .gap_1()
                        .child(
                            Button::new("confirm-clear")
                                .label("Confirm clear comments")
                                .on_click(
                                    cx.listener(|this, _, w, c| this.clear_review_comments(w, c)),
                                ),
                        )
                        .child(
                            Button::new("cancel-clear")
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, _, c| {
                                    this.clear_queue_request = None;
                                    c.notify();
                                })),
                        )
                } else {
                    h_flex().flex_wrap()
                        .p_2()
                        .gap_1()
                        .child(
                            Button::new("comment-selected-line")
                                .small().label("Line scope")
                                .disabled(self.anchor.is_none() || self.loading || self.navigation_file.is_some())
                                .on_click(cx.listener(|this,_,w,c| this.begin_comment(this.anchor.clone(),w,c))),
                        )
                        .child(
                            Button::new("discard-draft")
                                .small()
                                .label(if self.editing.is_some() {"Discard edit"}else{"Discard draft"})
                                .on_click(cx.listener(|this, _, w, c| {
                                    if this.editing.is_some() {this.cancel_edit(w,c);return;}
                                    this.comment.update(c, |s, c| s.set_value("", w, c));
                                    this.comment_anchor = None;
                                    this.comment_fingerprint = None;
                                    this.editing = None;
                                    this.pending_new = None;
                                    this.remember(c);
                                    c.notify();
                                })),
                        )
                        .child(
                            Button::new("clear-comments")
                                .small()
                                .label("Clear comments…")
                                .disabled(self.queue.comments.is_empty())
                                .on_click(cx.listener(|this, _, _, c| {
                                    this.clear_queue_request =
                                        this.snapshot.as_ref().map(|s| s.queue_id.clone());
                                    c.notify();
                                })),
                        )
                });
        root = root
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .w(px(self.preferences.sidebar_widths.files as f32))
                            .h_full()
                            .flex_shrink_0()
                            .child(sidebar),
                    )
                    .child(self.splitter(0, cx))
                    .child(center.size_full().flex_1().min_w_0())
                    .when(self.feedback_visible, |row| {
                        row.child(self.splitter(1, cx)).child(
                            div()
                                .w(px(self.preferences.sidebar_widths.feedback as f32))
                                .h_full()
                                .flex_shrink_0()
                                .child(feedback_panel),
                        )
                    }),
            )
            .child(
                div()
                    .p_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(if self.loading {
                        "Loading…".into()
                    } else {
                        format!(
                            "{} · {} comments · {} reviewed",
                            self.notice,
                            self.queue.comments.len(),
                            self.queue.reviewed.len()
                        )
                    }),
            );
        if self.quick_visible {
            let results = self.palette_files(cx);
            let count = results.len();
            if self.palette_scroll_pending {
                self.palette_scroll
                    .scroll_to_item(self.quick_index, ScrollStrategy::Nearest);
                self.palette_scroll_pending = false;
            }
            let cursor = self.quick_index;
            let query = self.quick_input.read(cx).value().to_string();
            let weak = cx.entity().downgrade();
            let choices = uniform_list("quick-file-results", count, move |range, _, cx| {
                range
                    .map(|ix| {
                        let file = &results[ix];
                        let id = file.id.clone();
                        let weak = weak.clone();
                        list::ListItem::new(SharedString::from(format!("quick-file-{id}")))
                            .h(px(32.))
                            .text_sm()
                            .selected(ix == cursor)
                            .accessibility_label(file.path.clone())
                            .child(
                                h_flex()
                                    .gap_2()
                                    .w_full()
                                    .min_w_0()
                                    .child(Icon::new(IconName::File).size(px(14.)).flex_shrink_0())
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(
                                                StyledText::new(file.path.clone()).with_highlights(
                                                    file_tree::matches(&file.path, &query)
                                                        .into_iter()
                                                        .map(|range| {
                                                            (
                                                                range,
                                                                HighlightStyle {
                                                                    background_color: Some(
                                                                        cx.theme().selection,
                                                                    ),
                                                                    font_weight: Some(
                                                                        FontWeight::BOLD,
                                                                    ),
                                                                    ..Default::default()
                                                                },
                                                            )
                                                        }),
                                                ),
                                            ),
                                    ),
                            )
                            .on_click(move |_, w, c| {
                                let _ = weak
                                    .update(c, |this, c| this.open_palette_file(id.clone(), w, c));
                            })
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            })
            .track_scroll(&self.palette_scroll)
            .flex_1()
            .min_h_0();
            root = root.relative().child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x00000055))
                    .flex()
                    .justify_center()
                    .pt(px(100.))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, w, c| this.close_palette(w, c)),
                    )
                    .child(
                        v_flex()
                            .key_context("PeerQuickOpen")
                            .w(px(760.))
                            .h(px(480.))
                            .p_3()
                            .gap_2()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_lg()
                            .shadow_lg()
                            .on_mouse_down(MouseButton::Left, |_, _, c| c.stop_propagation())
                            .child(
                                h_flex().child("Go to file").child(div().flex_1()).child(
                                    Button::new("close-quick-file")
                                        .ghost()
                                        .label("Esc · Close")
                                        .on_click(
                                            cx.listener(|this, _, w, c| this.close_palette(w, c)),
                                        ),
                                ),
                            )
                            .child(Input::new(&self.quick_input))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_h_0()
                                    .child(choices)
                                    .vertical_scrollbar(&self.palette_scroll),
                            )
                            .child(format!("{count} results · ↑ ↓ choose · Enter open")),
                    ),
            );
        }
        let _ = (window, &self.subscriptions);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::{files, DiffFile, FileContent, Preferences, SavedView};
    fn content(old: Option<&str>, new: Option<&str>) -> FileContent {
        FileContent {
            snapshot_id: "snapshot".into(),
            file_id: "file".into(),
            path: "目录/odd\tname.rs".into(),
            old_path: None,
            status: "modified".into(),
            kind: "text".into(),
            old_content: old.map(str::to_owned),
            new_content: new.map(str::to_owned),
            fingerprint: "fingerprint".into(),
            tab_width: 8,
            deferred: false,
            size: 0,
            metadata: String::new(),
        }
    }
    #[test]
    fn independent_edit_and_new_draft_completion_preserves_the_other_buffer() {
        let new = SavedView {
            body: "new draft".into(),
            fingerprint: Some("v1".into()),
            ..Default::default()
        };
        let edit = SavedView {
            body: "edited saved comment".into(),
            editing: Some("comment-id".into()),
            pending_new: Some(Box::new(new.clone())),
            ..Default::default()
        };
        let after_new = edit.after_submission(&new, &SavedView::default()).unwrap();
        assert_eq!(after_new.body, "edited saved comment");
        assert_eq!(after_new.editing.as_deref(), Some("comment-id"));
        assert_eq!(after_new.pending_new.as_ref().unwrap().body, "");
        let mut current = edit.clone();
        current.pending_new.as_mut().unwrap().body = "typed while edit save runs".into();
        let after_edit = current.after_submission(&edit, &new).unwrap();
        assert_eq!(after_edit.body, "typed while edit save runs");
        assert!(after_edit.editing.is_none());
        current.body = "newer edit".into();
        assert!(current.after_submission(&edit, &new).is_none());
        current.pending_new.as_mut().unwrap().body = "newer new draft".into();
        assert!(current
            .after_submission(&new, &SavedView::default())
            .is_none());
    }

    #[test]
    fn continuous_changes_default_respects_explicit_single_file_preference() {
        assert!(Preferences::default().all_files);
        assert!(serde_json::from_str::<Preferences>("{}").unwrap().all_files);
        assert!(
            !serde_json::from_str::<Preferences>(r#"{"allFiles":false}"#)
                .unwrap()
                .all_files
        );
    }

    #[test]
    fn native_parser_accepts_real_old_new_source_and_quoted_paths() {
        let c = content(Some("same\r\n\t旧😀\r\n"), Some("same\r\n\t新😀\r\nlast"));
        let models = files(&c, false).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].path().as_str(), c.path);
        assert_eq!(models[0].additions(), 2);
        assert_eq!(models[0].deletions(), 1);
    }
    #[test]
    fn additions_deletions_and_renames_keep_absent_sides() {
        let added = files(&content(None, Some("new\n")), false).unwrap();
        assert!(added[0].original_path().is_none());
        let deleted = files(&content(Some("old\n"), None), false).unwrap();
        assert!(deleted[0].modified_path().is_none());
        let mut renamed = content(Some("old\n"), Some("new\n"));
        renamed.old_path = Some("previous.rs".into());
        let models = files(&renamed, false).unwrap();
        assert_eq!(models[0].original_path().unwrap().as_str(), "previous.rs");
        assert_eq!(models[0].path().as_str(), renamed.path);
    }
    #[test]
    fn deferred_binary_and_empty_sources_do_not_invent_lines() {
        let mut c = content(None, Some(""));
        assert_eq!(files(&c, false).unwrap()[0].additions(), 0);
        c.deferred = true;
        assert!(files(&c, false).unwrap().is_empty());
        c.deferred = false;
        c.kind = "binary".into();
        assert!(files(&c, false).unwrap().is_empty());
    }
    #[test]
    fn multi_file_large_change_uses_actual_parser() {
        let mut all = Vec::new();
        for index in 0..100 {
            let old = (0..100).map(|n| format!("old {n}\n")).collect::<String>();
            let new = old.replace("old", "new");
            let mut c = content(Some(&old), Some(&new));
            c.path = format!("src/file{index}.rs");
            all.extend(files(&c, false).unwrap());
        }
        assert_eq!(all.len(), 100);
        assert_eq!(all.iter().map(DiffFile::additions).sum::<usize>(), 10_000);
        assert_eq!(all.iter().map(DiffFile::deletions).sum::<usize>(), 10_000);
    }
    #[test]
    fn full_old_new_context_preserves_multiline_comment_string_and_heredoc() {
        use gpui_kit::component::{diff::DiffSide, highlighter::SyntaxHighlighter, Rope};
        use std::{sync::atomic::AtomicBool, time::Duration};
        for (language, path, old, new, capture) in [
        (
            "rust",
            "context.rs",
            "/* start\ncontext 0\ncontext 1\ncontext 2\ncontext 3\ncontext 4\ncontext 5\ncontext 6\ncontext 7\ncontext 8\nold marker\nend */\nfn main() {}\n",
            "/* start\ncontext 0\ncontext 1\ncontext 2\ncontext 3\ncontext 4\ncontext 5\ncontext 6\ncontext 7\ncontext 8\nnew marker\nend */\nfn main() {}\n",
            "comment",
        ),
        (
            "python",
            "context.py",
            "message = \"\"\"start\ncontext 0\ncontext 1\ncontext 2\ncontext 3\ncontext 4\ncontext 5\ncontext 6\ncontext 7\ncontext 8\nold marker\nend\"\"\"\n",
            "message = \"\"\"start\ncontext 0\ncontext 1\ncontext 2\ncontext 3\ncontext 4\ncontext 5\ncontext 6\ncontext 7\ncontext 8\nnew marker\nend\"\"\"\n",
            "string",
        ),
        (
            "bash",
            "context.sh",
            "cat <<'EOF'\ncontext 0\ncontext 1\ncontext 2\ncontext 3\ncontext 4\ncontext 5\ncontext 6\ncontext 7\ncontext 8\nold marker\nEOF\n",
            "cat <<'EOF'\ncontext 0\ncontext 1\ncontext 2\ncontext 3\ncontext 4\ncontext 5\ncontext 6\ncontext 7\ncontext 8\nnew marker\nEOF\n",
            "string",
        ),
    ] {
        let mut content = content(Some(old), Some(new));
        content.path = path.into();
        let documents = files(&content, false).unwrap();
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].language().as_str(), language);
        let patch = peer_core::presentation::unified_patch(&content);
        assert!(patch
            .lines()
            .any(|line| line == format!(" {}", old.lines().next().unwrap())));
        for (side, expected, marker) in [
            (DiffSide::Original, old, "old marker"),
            (DiffSide::Modified, new, "new marker"),
        ] {
            let source = if side == DiffSide::Original {
                content.old_content.as_deref().unwrap()
            } else {
                content.new_content.as_deref().unwrap()
            };
            assert_eq!(source, expected, "{language}: complete context on {side:?}");
            let start = source.find(marker).unwrap();
            let captures = SyntaxHighlighter::new(language).capture_names_with_budget(
                &Rope::from_str(source),
                Duration::from_secs(2),
                &AtomicBool::new(false),
            );
            assert!(
                captures.iter().any(|(r, n)| r.start <= start
                    && r.end >= start + marker.len()
                    && n.starts_with(capture)),
                "{language}: {captures:?}"
            );
        }
    }
    }
}

#[cfg(test)]
mod review_coordinate_tests {
    use super::{legacy_draft, source_byte, Position, SavedView};
    #[test]
    fn lsp_utf16_ranges_respect_unicode_tabs_and_crlf() {
        let text = "\t中😀x\r\nlast\n";
        assert_eq!(
            source_byte(
                text,
                Position {
                    line: 0,
                    character: 4
                }
            ),
            Some(8)
        );
        assert_eq!(
            source_byte(
                text,
                Position {
                    line: 0,
                    character: 3
                }
            ),
            None
        );
        assert_eq!(
            source_byte(
                text,
                Position {
                    line: 1,
                    character: 2
                }
            ),
            Some(13)
        );
        assert_eq!(
            source_byte(
                text,
                Position {
                    line: 0,
                    character: 6
                }
            ),
            None
        );
        assert_eq!(
            source_byte(
                text,
                Position {
                    line: 2,
                    character: 0
                }
            ),
            Some(text.len())
        );
    }
    #[test]
    fn legacy_saved_view_can_restore_edit_backup() {
        let old: SavedView =
            serde_json::from_str(r#"{"body":"draft","side":"old","line":10}"#).unwrap();
        assert_eq!(old.line, Some(10));
        assert!(old.fingerprint.is_none());
        let edit = SavedView {
            body: "edit".into(),
            editing: Some("id".into()),
            pending_new: Some(Box::new(old)),
            ..Default::default()
        };
        let restored: SavedView =
            serde_json::from_value(serde_json::to_value(edit).unwrap()).unwrap();
        assert_eq!(restored.pending_new.unwrap().body, "draft");
    }
    #[test]
    fn save_completion_does_not_clear_reanchored_or_replaced_drafts() {
        let submitted = SavedView {
            body: "draft".into(),
            side: Some("old".into()),
            line: Some(10),
            fingerprint: Some("before".into()),
            ..Default::default()
        };
        let mut current = submitted.clone();
        current.scroll_offset = 99.;
        assert!(current.same_draft(&submitted));
        current.line = Some(20);
        assert!(!current.same_draft(&submitted));
        current = submitted.clone();
        current.body = "newer typing".into();
        assert!(!current.same_draft(&submitted));
        current = submitted.clone();
        current.fingerprint = Some("after".into());
        assert!(!current.same_draft(&submitted));
    }
    #[test]
    fn legacy_empty_current_view_does_not_hide_stale_draft_or_cross_paths() {
        let old = "a".repeat(64);
        let current = "b".repeat(64);
        let mut views = std::collections::BTreeMap::new();
        views.insert(format!("queue:path:{current}"), SavedView::default());
        views.insert(
            format!("queue:path:child:{old}"),
            SavedView {
                body: "other file".into(),
                ..Default::default()
            },
        );
        assert!(legacy_draft(&views, "queue:path", &current)
            .unwrap()
            .body
            .is_empty());
        views.insert(
            format!("queue:path:{old}"),
            SavedView {
                body: "stale draft".into(),
                side: Some("old".into()),
                line: Some(10),
                ..Default::default()
            },
        );
        let view = legacy_draft(&views, "queue:path", &current).unwrap();
        assert_eq!(view.body, "stale draft");
        assert_eq!(view.fingerprint, Some(old));
        assert_eq!(view.line, Some(10));
    }
}
