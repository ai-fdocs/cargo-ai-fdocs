use std::collections::HashMap;
use std::path::Path;

use toml::Value;

use crate::error::{AiDocsError, Result};

pub fn resolve_cargo_versions(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Err(AiDocsError::CargoLockNotFound);
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

#[cfg(test)]
mod tests {
    use super::resolve_cargo_versions;
    use std::fs;

    #[test]
    fn parses_lockfile_packages_into_map() {
        let tmp = std::env::temp_dir().join(format!(
            "ai-fdocs-resolver-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("t")
        ));
        let _ = fs::remove_file(&tmp);

        let content = r#"
[[package]]
name = "serde"
version = "1.0.210"

[[package]]
name = "tokio"
version = "1.44.0"
"#;
        fs::write(&tmp, content).expect("write lockfile");

        let versions = resolve_cargo_versions(&tmp).expect("resolve versions");
        assert_eq!(versions.get("serde"), Some(&"1.0.210".to_string()));
        assert_eq!(versions.get("tokio"), Some(&"1.44.0".to_string()));

        let _ = fs::remove_file(&tmp);
    }
}
