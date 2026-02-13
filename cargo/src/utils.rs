use crate::error::Result;
use chrono::{NaiveDate, Utc};

/// Semver-aware version comparison. Returns true if new_v > current_best.
pub fn is_version_better(new_v: &str, current_best: Option<&str>) -> bool {
    let Some(best) = current_best else {
        return true;
    };

    // Very simple semver-ish comparison: split by dots and try to compare numbers
    let new_parts: Vec<&str> = new_v.split('.').collect();
    let best_parts: Vec<&str> = best.split('.').collect();

    for i in 0..new_parts.len().max(best_parts.len()) {
        let n = new_parts.get(i).and_then(|&s| s.parse::<u32>().ok());
        let b = best_parts.get(i).and_then(|&s| s.parse::<u32>().ok());

        match (n, b) {
            (Some(nv), Some(bv)) if nv != bv => return nv > bv,
            (Some(_), None) => return true,
            (None, Some(_)) => return false,
            _ => {
                let ns = new_parts.get(i).unwrap_or(&"");
                let bs = best_parts.get(i).unwrap_or(&"");
                if ns != bs {
                    return ns > bs;
                }
            }
        }
    }

    false
}

/// Rounds down to the nearest char boundary.
pub fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Checks if a cache entry is still valid according to TTL.
pub fn is_latest_cache_fresh(fetched_at: &str, latest_ttl_hours: usize) -> bool {
    let Ok(date) = NaiveDate::parse_from_str(fetched_at, "%Y-%m-%d") else {
        return false;
    };

    let Some(fetched_dt) = date.and_hms_opt(0, 0, 0) else {
        return false;
    };

    let now = Utc::now().naive_utc();
    let age = now - fetched_dt;
    age.num_hours() < latest_ttl_hours as i64
}
