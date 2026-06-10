use std::path::Path;

use serde_json::{json, Value};

fn percent_encode_filename(input: &str) -> String {
    // Encode everything except RFC3986 "unreserved" characters.
    // This matches the intent of SST/Python `quote()` without pulling deps.
    fn is_unreserved(b: u8) -> bool {
        matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~')
    }

    let mut out = String::new();
    for &b in input.as_bytes() {
        if is_unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn project_font_family_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

    // Normalize obvious separators.
    let normalized = stem.replace(['_', '-'], " ");
    let chars: Vec<char> = normalized.chars().collect();

    let mut out = String::new();
    for i in 0..chars.len() {
        let c = chars[i];
        if c == ' ' {
            out.push(' ');
            continue;
        }

        if i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();

            let boundary = (c.is_ascii_uppercase() && prev.is_ascii_lowercase())
                || (c.is_ascii_uppercase()
                    && prev.is_ascii_uppercase()
                    && next.map(|n| n.is_ascii_lowercase()).unwrap_or(false))
                || (c.is_ascii_digit() && prev.is_ascii_lowercase())
                || (c.is_ascii_uppercase() && prev.is_ascii_digit());

            if boundary {
                out.push(' ');
            }
        }

        out.push(c);
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn font_extension_format(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "woff2" => Some("woff2"),
        "woff" => Some("woff"),
        "ttf" => Some("truetype"),
        "otf" => Some("opentype"),
        _ => None,
    }
}

/// Returns one catalog entry per project-local font file.
pub fn list_project_font_entries(project_fonts_dir: &Path) -> Vec<Value> {
    let _ = std::fs::create_dir_all(project_fonts_dir);

    let Ok(dir_entries) = std::fs::read_dir(project_fonts_dir) else {
        return Vec::new();
    };

    let mut files: Vec<std::path::PathBuf> = dir_entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();

    files.sort_by_key(|p| {
        p.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
    });

    let mut entries: Vec<Value> = Vec::new();
    for path in files {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let Some(format) = font_extension_format(ext) else {
            continue;
        };

        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(filename);
        let label = project_font_family_name(&path);
        let family = format!("\"{label}\"");
        let url = format!("/project-fonts/{}", percent_encode_filename(filename));

        entries.push(json!({
            "id": format!("project-{}", stem.to_ascii_lowercase()),
            "label": label,
            "family": family,
            "source": "project_local",
            "url": url,
            "filename": filename,
            "format": format,
        }));
    }

    entries
}

pub fn build_font_catalog(project_fonts_dir: &Path) -> Value {
    // Port of SST/Python `backend/core/font_catalog.py` fallback entries.
    let fallback = vec![
        json!({
            "id": "fallback-segoe-ui",
            "label": "Segoe UI",
            "family": "\"Segoe UI\", Tahoma, Geneva, Verdana, sans-serif",
            "source": "fallback",
        }),
        json!({
            "id": "fallback-yu-gothic-ui",
            "label": "Yu Gothic UI",
            "family": "\"Yu Gothic UI\", \"Yu Gothic\", Meiryo, sans-serif",
            "source": "fallback",
        }),
        json!({
            "id": "fallback-biz-udpgothic",
            "label": "BIZ UDPGothic",
            "family": "\"BIZ UDPGothic\", \"Yu Gothic UI\", Meiryo, sans-serif",
            "source": "fallback",
        }),
        json!({
            "id": "fallback-meiryo",
            "label": "Meiryo",
            "family": "\"Meiryo\", \"Yu Gothic UI\", sans-serif",
            "source": "fallback",
        }),
        json!({
            "id": "fallback-arial",
            "label": "Arial",
            "family": "Arial, \"Segoe UI\", sans-serif",
            "source": "fallback",
        }),
        json!({
            "id": "fallback-verdana",
            "label": "Verdana",
            "family": "Verdana, \"Segoe UI\", sans-serif",
            "source": "fallback",
        }),
        json!({
            "id": "fallback-trebuchet",
            "label": "Trebuchet MS",
            "family": "\"Trebuchet MS\", \"Segoe UI\", sans-serif",
            "source": "fallback",
        }),
        json!({
            "id": "fallback-ud-digi",
            "label": "UD Digi Kyokasho",
            "family": "\"UD Digi Kyokasho NK-R\", \"Yu Gothic UI\", Meiryo, sans-serif",
            "source": "fallback",
        }),
    ];

    json!({
        "project_fonts_dir": project_fonts_dir.display().to_string(),
        "project_local": list_project_font_entries(project_fonts_dir),
        "fallback": fallback,
    })
}

/// Port of SST `build_project_fonts_stylesheet` (shared with catalog).
pub fn build_project_fonts_stylesheet(project_fonts_dir: &Path) -> String {
    let entries = list_project_font_entries(project_fonts_dir);
    if entries.is_empty() {
        return "/* No project-local fonts found in the fonts folder yet. */\n".into();
    }

    let mut rules = Vec::new();
    for entry in entries {
        let label = entry
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let url = entry
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let format = entry
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let family = label.replace('"', "\\\"");

        rules.push(format!(
            "@font-face {{\n  font-family: \"{family}\";\n  src: url(\"{url}\") format(\"{format}\");\n  font-display: swap;\n}}"
        ));
    }

    format!("{}\n", rules.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dir_returns_comment() {
        let dir = std::env::temp_dir().join(format!("voicesub-fonts-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let css = build_project_fonts_stylesheet(&dir);
        assert!(css.contains("No project-local fonts"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
