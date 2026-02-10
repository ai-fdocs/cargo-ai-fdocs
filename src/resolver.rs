use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::{AiDocsError, Result};

#[derive(Debug, Deserialize)]
struct LockFile {
    package: Vec<Package>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    version: String,
}

pub struct LockResolver;

impl LockResolver {
    /// Reads Cargo.lock and returns HashMap<crate_name, version>
    pub fn resolve(path: &Path) -> Result<HashMap<String, String>> {
        if !path.exists() {
            return Err(AiDocsError::CargoLockNotFound);
        }

        let content = std::fs::read_to_string(path)?;

        let lock: LockFile =
            toml::from_str(&content).map_err(|e| AiDocsError::CargoLockParse(e.to_string()))?;

        let mut map = HashMap::new();
        for pkg in lock.package {
            // MVP behavior: if multiple versions exist for one crate,
            // keep the last occurrence.
            map.insert(pkg.name, pkg.version);
        }

        Ok(map)
    }
}
