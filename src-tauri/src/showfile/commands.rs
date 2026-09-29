//! Tauri commands for New/Open/Save/Save As/Autosave/Recent.
//!
//! File dialogs run natively in Rust via `rfd` rather than through a JS-facing
//! plugin, so no extra Tauri capability grants are needed for them.

use std::fs;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use super::migrate::migrate;
use super::schema::{RecentShowEntry, ShowFile, CURRENT_SCHEMA_VERSION};

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn recent_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_data_dir(app)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("recent.json"))
}

fn read_recent(app: &AppHandle) -> Vec<RecentShowEntry> {
    let Ok(path) = recent_file_path(app) else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn write_recent(app: &AppHandle, entries: &[RecentShowEntry]) {
    if let Ok(path) = recent_file_path(app) {
        if let Ok(text) = serde_json::to_string_pretty(entries) {
            let _ = fs::write(path, text);
        }
    }
}

fn touch_recent(app: &AppHandle, path: &str, name: &str) {
    let mut entries = read_recent(app);
    entries.retain(|e| e.path != path);
    entries.insert(
        0,
        RecentShowEntry {
            path: path.to_string(),
            name: name.to_string(),
            opened_at: chrono::Utc::now().to_rfc3339(),
        },
    );
    entries.truncate(10);
    write_recent(app, &entries);
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.trim().is_empty() {
        "Untitled Show".to_string()
    } else {
        cleaned
    }
}

#[tauri::command]
pub fn showfile_new() -> ShowFile {
    ShowFile::new_default("Untitled Show")
}

#[tauri::command]
pub fn showfile_open(app: AppHandle, path: Option<String>) -> Result<(ShowFile, String), String> {
    let chosen = match path {
        Some(p) => Some(PathBuf::from(p)),
        None => rfd::FileDialog::new()
            .add_filter("LumenDMX show", &["lumen"])
            .pick_file(),
    };
    let Some(chosen) = chosen else {
        return Err("No file selected".to_string());
    };

    let text = fs::read_to_string(&chosen)
        .map_err(|e| format!("Failed to read {}: {e}", chosen.display()))?;
    let raw: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid show file: {e}"))?;
    let show = migrate(raw)?;

    let path_str = chosen.to_string_lossy().to_string();
    touch_recent(&app, &path_str, &show.name);
    Ok((show, path_str))
}

/// Writes `show` to `path` (Save), or prompts for a location first when `path`
/// is `None` (Save As / first save). Returns the path actually used and the
/// show with `schemaVersion`/`modifiedAt` refreshed, so the frontend's copy
/// stays in sync.
#[tauri::command]
pub fn showfile_save(
    app: AppHandle,
    mut show: ShowFile,
    path: Option<String>,
) -> Result<(String, ShowFile), String> {
    let chosen = match path {
        Some(p) => PathBuf::from(p),
        None => rfd::FileDialog::new()
            .add_filter("LumenDMX show", &["lumen"])
            .set_file_name(format!("{}.lumen", sanitize_filename(&show.name)))
            .save_file()
            .ok_or_else(|| "Save cancelled".to_string())?,
    };

    show.schema_version = CURRENT_SCHEMA_VERSION;
    show.modified_at = chrono::Utc::now().to_rfc3339();

    write_show_file(&chosen, &show)?;

    let path_str = chosen.to_string_lossy().to_string();
    touch_recent(&app, &path_str, &show.name);
    Ok((path_str, show))
}

fn write_show_file(path: &Path, show: &ShowFile) -> Result<(), String> {
    let text = serde_json::to_string_pretty(show).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, text).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// Writes a timestamped snapshot into the app data dir's autosave folder for
/// this show and prunes anything past the last 10. Never touches the user's
/// own save file.
#[tauri::command]
pub fn showfile_autosave(app: AppHandle, show: ShowFile) -> Result<(), String> {
    let dir = app_data_dir(&app)?.join("autosave").join(&show.id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let filename = format!(
        "autosave-{}.lumen",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%.3f")
    );
    write_show_file(&dir.join(filename), &show)?;
    prune_autosaves(&dir, 10);
    Ok(())
}

fn prune_autosaves(dir: &Path, keep: usize) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = read_dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "lumen"))
        .collect();
    files.sort();
    if files.len() > keep {
        for old in &files[..files.len() - keep] {
            let _ = fs::remove_file(old);
        }
    }
}

#[tauri::command]
pub fn showfile_recent(app: AppHandle) -> Vec<RecentShowEntry> {
    read_recent(&app)
}
