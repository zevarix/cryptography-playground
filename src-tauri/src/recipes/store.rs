//! Local, offline recipe storage under Tauri's application data directory.
//! User recipe IDs are constrained and never used as arbitrary paths.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

use super::Recipe;

const MAX_SAVED: usize = 200;
const MAX_FILE_BYTES: u64 = 64 * 1024;

fn directory(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot locate app data directory: {e}"))?;
    Ok(root.join("recipes"))
}
fn path_for(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !id.starts_with("user-")
        || id.len() > 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        return Err("User recipe IDs must start with user- and contain only lowercase letters, digits and hyphens.".into());
    }
    Ok(root.join(format!("{id}.json")))
}
fn list_in(root: &Path) -> Result<Vec<Recipe>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut recipes = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| format!("Cannot list recipes: {e}"))? {
        let entry = entry.map_err(|e| format!("Cannot inspect recipe entry: {e}"))?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        if recipes.len() >= MAX_SAVED {
            return Err("Too many saved recipes (limit 200).".into());
        }
        if entry
            .metadata()
            .map_err(|e| format!("Cannot inspect recipe metadata: {e}"))?
            .len()
            > MAX_FILE_BYTES
        {
            return Err(format!(
                "Recipe file exceeds the size limit: {}",
                path.display()
            ));
        }
        let data = fs::read_to_string(&path).map_err(|e| format!("Cannot read recipe: {e}"))?;
        let recipe: Recipe = serde_json::from_str(&data)
            .map_err(|e| format!("Invalid saved recipe {}: {e}", path.display()))?;
        recipe.validate()?;
        if path_for(root, &recipe.id)? != path {
            return Err(format!(
                "Recipe file name and ID disagree: {}",
                path.display()
            ));
        }
        recipes.push(recipe);
    }
    recipes.sort_by_key(|r| r.name.to_lowercase());
    Ok(recipes)
}
fn save_in(root: &Path, recipe: Recipe) -> Result<Recipe, String> {
    recipe.validate()?;
    let dest = path_for(root, &recipe.id)?;
    fs::create_dir_all(root).map_err(|e| format!("Cannot create recipe directory: {e}"))?;
    if !dest.exists() && list_in(root)?.len() >= MAX_SAVED {
        return Err("Saved recipe limit reached (200).".into());
    }
    let data =
        serde_json::to_vec_pretty(&recipe).map_err(|e| format!("Cannot serialize recipe: {e}"))?;
    if data.len() as u64 > MAX_FILE_BYTES {
        return Err("Recipe exceeds the 64 KiB storage limit.".into());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("System time unavailable: {e}"))?
        .as_nanos();
    let stage = root.join(format!(
        ".{}.{}.{}.tmp",
        recipe.id,
        std::process::id(),
        nonce
    ));
    let written = (|| -> Result<(), String> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| format!("Cannot create temporary recipe: {e}"))?;
        file.write_all(&data)
            .map_err(|e| format!("Cannot write recipe: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Cannot flush recipe: {e}"))?;
        // Windows and macOS support replacing an existing regular file with
        // rename. A failure leaves the previous saved JSON untouched.
        fs::rename(&stage, &dest).map_err(|e| format!("Cannot replace saved recipe: {e}"))
    })();
    if written.is_err() {
        let _ = fs::remove_file(&stage);
    }
    written?;
    let observed = fs::read(&dest).map_err(|e| format!("Cannot read back saved recipe: {e}"))?;
    if observed != data {
        return Err("Saved recipe differs from the serialized data.".into());
    }
    let decoded: Recipe = serde_json::from_slice(&observed)
        .map_err(|e| format!("Saved recipe cannot be parsed on readback: {e}"))?;
    if decoded.id != recipe.id {
        return Err("Saved recipe identity changed on readback.".into());
    }
    Ok(decoded)
}
fn delete_in(root: &Path, id: &str) -> Result<(), String> {
    let path = path_for(root, id)?;
    fs::remove_file(&path).map_err(|e| format!("Cannot delete saved recipe: {e}"))
}
pub fn list(app: &AppHandle) -> Result<Vec<Recipe>, String> {
    list_in(&directory(app)?)
}
pub fn save(app: &AppHandle, recipe: Recipe) -> Result<Recipe, String> {
    save_in(&directory(app)?, recipe)
}
pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    delete_in(&directory(app)?, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_validation_blocks_traversal_and_builtin_overwrite() {
        let root = Path::new("recipes");
        assert!(path_for(root, "builtin-001").is_err());
        assert!(path_for(root, "user-../../private").is_err());
        assert!(path_for(root, "user-work").is_ok());
    }
    #[test]
    fn creates_updates_reopens_and_deletes_user_recipe() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cryptoplay-recipe-test-{}-{unique}",
            std::process::id()
        ));
        assert!(!root.exists());
        let mut recipe = super::super::builtins().unwrap().remove(2);
        recipe.id = "user-test-replace".into();
        recipe.name = "First version".into();
        save_in(&root, recipe.clone()).unwrap();
        assert_eq!(list_in(&root).unwrap()[0].name, "First version");
        recipe.name = "Updated version".into();
        save_in(&root, recipe.clone()).unwrap();
        let reopened = list_in(&root).unwrap();
        assert_eq!(reopened.len(), 1);
        assert_eq!(reopened[0].name, "Updated version");
        assert_eq!(reopened[0].steps.len(), recipe.steps.len());
        delete_in(&root, &recipe.id).unwrap();
        assert!(list_in(&root).unwrap().is_empty());
        fs::remove_dir(&root).unwrap();
    }
}
