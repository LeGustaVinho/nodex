use std::path::{Component, Path, PathBuf};

use crate::Desktop;

#[tauri::command]
pub(crate) async fn project_file_action(
    desktop: tauri::State<'_, Desktop>,
    expected_root: String,
    relative_path: String,
    operation: String,
) -> Result<(), String> {
    let path = {
        let project = desktop.state.project.lock().map_err(|_| "The project is unavailable.")?;
        let root = project.root();
        check_project_root(project.is_open(), root, &expected_root)?;
        if project.file_ir(&relative_path).is_none() {
            return Err("This file is no longer indexed by the project.".into());
        }
        resolve_file(root, Path::new(&relative_path))?
    };

    let result = match operation.as_str() {
        "open" => tauri_plugin_opener::open_path(path, None::<&str>),
        "reveal" => tauri_plugin_opener::reveal_item_in_dir(path),
        _ => return Err("Unknown file action.".into()),
    };
    result.map_err(|error| error.to_string())
}

fn check_project_root(open: bool, root: &Path, expected_root: &str) -> Result<(), String> {
    if !open || root.to_string_lossy() != expected_root {
        return Err("The active project has changed.".into());
    }
    Ok(())
}

fn resolve_file(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if relative.as_os_str().is_empty() || relative.components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err("This path is not a project file.".into());
    }
    let canonical_root = root.canonicalize().map_err(|_| "The project folder is unavailable.")?;
    let file = root.join(relative).canonicalize().map_err(|_| "The file is missing or inaccessible.")?;
    let resolved_relative = file.strip_prefix(&canonical_root).map_err(|_| "The file is outside the project.")?;
    if !file.is_file() {
        return Err("The path no longer points to a file.".into());
    }
    // Keep the project's shell-friendly root spelling, including on Windows.
    Ok(root.join(resolved_relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_request_from_a_previous_project() {
        assert!(check_project_root(true, Path::new("new"), "old").is_err());
        assert!(check_project_root(false, Path::new(""), "").is_err());
    }

    #[test]
    fn resolves_only_existing_files_inside_the_project() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(resolve_file(root, Path::new("src/main.rs")).unwrap(), root.join("src").join("main.rs"));
        assert!(resolve_file(root, Path::new("../nodex-core/Cargo.toml")).is_err());
        assert!(resolve_file(root, &root.join("Cargo.toml")).is_err());
        assert!(resolve_file(root, Path::new("src")).is_err());
        assert!(resolve_file(root, Path::new("missing.ts")).is_err());
    }
}
