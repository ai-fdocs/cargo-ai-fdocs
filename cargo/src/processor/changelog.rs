use regex::Regex;
use tracing::debug;

/// Truncate changelog to entries around the current version:
/// keep current section(s) + one previous minor series.
pub fn truncate_changelog(content: &str, current_version: &str) -> String {
    let heading_re = Regex::new(r"(?m)^#{1,3}\s+.*?\b?\[?v?(\d+\.\d+\.\d+(?:-[\w.]+)?)\]?\b")
        .expect("valid changelog heading regex");

    let matches: Vec<(usize, String)> = heading_re
        .captures_iter(content)
        .filter_map(|cap| {
            let version = cap.get(1)?.as_str().to_string();
            let pos = cap.get(0)?.start();
            Some((pos, version))
        })
        .collect();

    if matches.is_empty() {
        debug!("No version headings found in CHANGELOG, returning as-is.");
        return content.to_string();
    }

    let current_minor = parse_minor(current_version);
    let mut found_current = false;
    let mut found_previous_minor = false;
    let mut cut_position: Option<usize> = None;

    for (pos, ver) in &matches {
        let ver_minor = parse_minor(ver);

        if ver == current_version {
            found_current = true;
            continue;
        }

        if found_current && !found_previous_minor {
            if ver_minor != current_minor || current_minor.is_none() {
                found_previous_minor = true;
                continue;
            }
            continue;
        }

        if found_previous_minor {
            cut_position = Some(*pos);
            break;
        }
    }

    if !found_current && matches.len() > 2 {
        cut_position = Some(matches[2].0);
    }

    match cut_position {
        Some(pos) => {
            let truncated = &content[..pos];
            format!(
                "{}\n---\n\n*[Earlier entries truncated by ai-fdocs]*\n",
                truncated.trim_end()
            )
        }
        None => content.to_string(),
    }
}

fn parse_minor(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_keeps_current_and_previous() {
        let changelog = r#"# Changelog

## 0.13.1 - 2024-01-15
- Fix bug

## 0.13.0 - 2024-01-01
- New feature

## 0.12.0 - 2023-12-01
- Old feature

## 0.11.0 - 2023-11-01
- Ancient feature
"#;
        let result = truncate_changelog(changelog, "0.13.1");
        assert!(result.contains("0.13.1"));
        assert!(result.contains("0.13.0"));
        assert!(result.contains("0.12.0"));
        assert!(!result.contains("0.11.0"));
        assert!(result.contains("[Earlier entries truncated by ai-fdocs]"));
    }

    #[test]
    fn test_no_version_headings_returns_as_is() {
        let content = "Just some text without versions.";
        let result = truncate_changelog(content, "1.0.0");
        assert_eq!(result, content);
    }
}
