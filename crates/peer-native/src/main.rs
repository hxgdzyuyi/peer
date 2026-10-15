#[cfg(test)]
mod highlight_tests;
mod themes;
mod workspace;
use gpui_kit::*;
use peer_core::Inspector;
use std::{path::PathBuf, sync::Arc};

fn resolve_data_directory(
    peer: Option<std::ffi::OsString>,
    legacy: Option<std::ffi::OsString>,
    platform: Option<PathBuf>,
) -> Result<PathBuf, String> {
    peer.or(legacy)
        .map(PathBuf::from)
        .or_else(|| platform.map(|p| p.join("dev.repoinspector.app")))
        .ok_or_else(|| "Cannot determine the application data directory".into())
}
fn data_directory() -> Result<PathBuf, String> {
    resolve_data_directory(
        std::env::var_os("PEER_DATA_DIR"),
        std::env::var_os("REPO_INSPECTOR_DATA_DIR"),
        dirs::data_dir(),
    )
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Peer: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let core = Arc::new(Inspector::new(data_directory()?)?);
    // Validate existing data before opening a window; malformed stores are never reset.
    let preferences = core.store.read()?.preferences;
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?,
    );
    let path = std::env::args()
        .nth(1)
        .or_else(|| preferences.last_repository.clone());
    let shutdown_core = core.clone();
    let shutdown_runtime = runtime.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            workspace::init(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1440.), px(900.)), cx)),
                window_min_size: Some(size(px(1000.), px(640.))),
                app_id: Some("dev.repoinspector.app".into()),
                ..Default::default()
            };
            match gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| workspace::Workspace::new(core, runtime, preferences, path, window, cx))
            }) {
                Ok(_) => cx.activate(true),
                Err(error) => {
                    eprintln!("Cannot open native window: {error}");
                    cx.quit();
                }
            }
        });
    shutdown_runtime.block_on(shutdown_core.shutdown_navigation());
    Ok(())
}

#[cfg(test)]
mod storage_tests {
    use super::resolve_data_directory;
    use std::path::PathBuf;
    #[test]
    fn legacy_storage_identifier_and_environment_precedence_are_preserved() {
        let platform = Some(PathBuf::from("platform"));
        assert_eq!(
            resolve_data_directory(Some("peer".into()), Some("legacy".into()), platform.clone())
                .unwrap(),
            PathBuf::from("peer")
        );
        assert_eq!(
            resolve_data_directory(None, Some("legacy".into()), platform.clone()).unwrap(),
            PathBuf::from("legacy")
        );
        assert_eq!(
            resolve_data_directory(None, None, platform).unwrap(),
            PathBuf::from("platform/dev.repoinspector.app")
        );
        assert!(resolve_data_directory(None, None, None).is_err());
    }
}
