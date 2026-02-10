use std::collections::HashMap;
use std::path::Path;

use toml::Value;

use crate::error::{AiDocsError, Result};

pub fn resolve_cargo_versions(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Err(AiDocsError::CargoLockNotFound(path.to_path_buf()));
    }

    let content = std::fs::read_to_string(path)?;
    let value: Value =
        toml::from_str(&content).map_err(|e| AiDocsError::CargoLockParse(e.to_string()))?;

    let mut versions = HashMap::new();
    let packages = value
        .get("package")
        .and_then(Value::as_array)
        .ok_or_else(|| AiDocsError::CargoLockParse("`package` array is missing".to_string()))?;

    for pkg in packages {
        if let (Some(name), Some(version)) = (
            pkg.get("name").and_then(Value::as_str),
            pkg.get("version").and_then(Value::as_str),
        ) {
            versions.insert(name.to_string(), version.to_string());
        }
    }

    Ok(versions)
}
