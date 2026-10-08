use peer_core::{types::*, Inspector};
use tauri::{Emitter, Manager, State};

#[tauri::command]
async fn bootstrap(core: State<'_, Inspector>) -> Result<serde_json::Value, String> {
    let data = core.store.read()?;
    Ok(serde_json::json!({ "preferences": data.preferences, "themes": data.themes }))
}
#[tauri::command]
async fn save_preferences(
    core: State<'_, Inspector>,
    preferences: Preferences,
) -> Result<(), String> {
    core.save_preferences(preferences)
}
#[tauri::command]
async fn open_repository(
    core: State<'_, Inspector>,
    path: String,
    comparison: Option<Comparison>,
) -> Result<Snapshot, String> {
    core.open_repository(&path, comparison).await
}
#[tauri::command]
async fn read_file(
    core: State<'_, Inspector>,
    snapshot_id: String,
    file_id: String,
    force: bool,
) -> Result<FileContent, String> {
    core.read_file(&snapshot_id, &file_id, force).await
}
#[tauri::command]
async fn list_branches(
    core: State<'_, Inspector>,
    snapshot_id: String,
) -> Result<Vec<String>, String> {
    core.branches(&snapshot_id).await
}
#[tauri::command]
async fn get_queue(core: State<'_, Inspector>, snapshot_id: String) -> Result<ReviewQueue, String> {
    core.queue(&snapshot_id).await
}
#[tauri::command]
async fn add_comment(core: State<'_, Inspector>, comment: NewComment) -> Result<Comment, String> {
    core.add_comment(comment).await
}
#[tauri::command]
async fn update_comment(
    core: State<'_, Inspector>,
    queue_id: String,
    id: String,
    body: String,
) -> Result<(), String> {
    core.update_comment(&queue_id, &id, &body)
}
#[tauri::command]
async fn delete_comments(
    core: State<'_, Inspector>,
    queue_id: String,
    id: Option<String>,
) -> Result<(), String> {
    core.delete_comments(&queue_id, id.as_deref())
}
#[tauri::command]
async fn set_reviewed(
    core: State<'_, Inspector>,
    snapshot_id: String,
    file_id: String,
    fingerprint: String,
    reviewed: bool,
) -> Result<(), String> {
    core.set_reviewed(&snapshot_id, &file_id, &fingerprint, reviewed)
        .await
}
#[tauri::command]
async fn export_queue(core: State<'_, Inspector>, snapshot_id: String) -> Result<String, String> {
    core.export(&snapshot_id).await
}
#[tauri::command]
async fn save_export(
    core: State<'_, Inspector>,
    snapshot_id: String,
    path: String,
) -> Result<(), String> {
    let text = core.export(&snapshot_id).await?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}
#[tauri::command]
async fn import_theme(core: State<'_, Inspector>, path: String) -> Result<ImportedTheme, String> {
    core.import_theme(&path)
}
#[tauri::command]
async fn remove_theme(core: State<'_, Inspector>, id: String) -> Result<(), String> {
    core.remove_theme(&id)
}

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(feature = "desktop-tests")]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let directory = std::env::var_os("PEER_DATA_DIR")
                .or_else(|| std::env::var_os("REPO_INSPECTOR_DATA_DIR"))
                .map(std::path::PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            app.manage(Inspector::new(directory)?);
            use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
            let open =
                MenuItem::with_id(app, "open", "Open Repository…", true, Some("CmdOrCtrl+O"))?;
            let settings =
                MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
            let refresh = MenuItem::with_id(
                app,
                "refresh",
                "Refresh Repository",
                true,
                Some("CmdOrCtrl+R"),
            )?;
            let about = PredefinedMenuItem::about(app, None, None)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit = PredefinedMenuItem::quit(app, None)?;
            let app_menu =
                Submenu::with_items(app, "Peer", true, &[&about, &settings, &separator, &quit])?;
            let file_menu = Submenu::with_items(app, "File", true, &[&open, &refresh])?;
            let edit_menu = Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?;
            app.set_menu(Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu])?)?;
            app.on_menu_event(|app, event| {
                let _ = app.emit("app-menu", event.id().as_ref());
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            save_preferences,
            open_repository,
            read_file,
            list_branches,
            get_queue,
            add_comment,
            update_comment,
            delete_comments,
            set_reviewed,
            export_queue,
            save_export,
            import_theme,
            remove_theme
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Peer");
}
