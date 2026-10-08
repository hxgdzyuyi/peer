use crate::{hash, types::ImportedTheme};
use serde_json::Value;

fn valid_color(value: &str) -> bool {
    value.starts_with('#')
        && [4, 5, 7, 9].contains(&value.len())
        && value[1..].bytes().all(|c| c.is_ascii_hexdigit())
}

pub fn import_theme(text: &str, fallback_name: &str) -> Result<ImportedTheme, String> {
    if text.len() > 2 * 1024 * 1024 {
        return Err("Theme files must be smaller than 2 MiB".into());
    }
    // JSON5 accepts JSONC comments and trailing commas; the normalized result is strict JSON.
    let mut value: Value = json5::from_str(text).map_err(|e| format!("Invalid theme JSON: {e}"))?;
    let obj = value
        .as_object_mut()
        .ok_or("A theme must be a JSON object")?;
    if obj.contains_key("include") {
        return Err("This theme uses an external include. Import a self-contained theme; VS Code's ‘Generate Color Theme from Current Settings’ can export one.".into());
    }
    if obj.get("tokenColors").is_some_and(Value::is_string) {
        return Err(
            "External token-color files are not supported. Import a self-contained theme.".into(),
        );
    }
    if !obj.contains_key("colors") && !obj.contains_key("tokenColors") {
        return Err("No VS Code colors or tokenColors were found in this file".into());
    }
    if let Some(colors) = obj.get("colors") {
        let colors = colors.as_object().ok_or("colors must be an object")?;
        for (key, color) in colors {
            if !color.as_str().is_some_and(valid_color) {
                return Err(format!(
                    "Invalid color for {key}; use a hex color such as #aabbcc or #aabbcc80"
                ));
            }
        }
    }
    if let Some(tokens) = obj.get("tokenColors") {
        for token in tokens.as_array().ok_or("tokenColors must be an array")? {
            let settings = token
                .get("settings")
                .and_then(Value::as_object)
                .ok_or("Each token rule must have a settings object")?;
            if let Some(scope) = token.get("scope") {
                if !(scope.is_string()
                    || scope
                        .as_array()
                        .is_some_and(|a| a.iter().all(Value::is_string)))
                {
                    return Err("Token scopes must be strings or arrays of strings".into());
                }
            }
            for key in ["foreground", "background"] {
                if let Some(color) = settings.get(key) {
                    if !color.as_str().is_some_and(valid_color) {
                        return Err(format!("Invalid token {key} color"));
                    }
                }
            }
            if let Some(style) = settings.get("fontStyle") {
                if !style.as_str().is_some_and(|s| {
                    s.split_whitespace()
                        .all(|t| ["italic", "bold", "underline", "strikethrough"].contains(&t))
                }) {
                    return Err("Unsupported token fontStyle".into());
                }
            }
        }
    }
    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_name)
        .to_string();
    let kind = match obj.get("type").and_then(Value::as_str) {
        Some("light" | "hcLight") => "light",
        Some("dark" | "hcDark" | "hc") => "dark",
        Some(_) => return Err("Theme type must be light or dark".into()),
        None => {
            let background = obj
                .get("colors")
                .and_then(|c| c.get("editor.background"))
                .and_then(Value::as_str)
                .unwrap_or("#1e1e1e");
            let chars: Vec<char> = background[1..].chars().collect();
            let hex: String = if chars.len() <= 4 {
                chars[..3].iter().flat_map(|c| [*c, *c]).collect()
            } else {
                chars[..6].iter().collect()
            };
            let rgb = u32::from_str_radix(&hex, 16).unwrap_or(0);
            if (((rgb >> 16) & 255) * 299 + ((rgb >> 8) & 255) * 587 + (rgb & 255) * 114) > 128000 {
                "light"
            } else {
                "dark"
            }
        }
    }
    .to_string();
    let id = format!("custom-{}", &hash(text.as_bytes())[..20]);
    let colors = obj
        .entry("colors")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .expect("colors validated above");
    colors.entry("editor.background").or_insert_with(|| {
        Value::String(
            if kind == "light" {
                "#ffffff"
            } else {
                "#0d1117"
            }
            .into(),
        )
    });
    let foreground = colors.get("foreground").cloned().unwrap_or_else(|| {
        Value::String(
            if kind == "light" {
                "#24292f"
            } else {
                "#e6edf3"
            }
            .into(),
        )
    });
    colors.entry("editor.foreground").or_insert(foreground);
    obj.insert("name".into(), Value::String(id.clone()));
    obj.insert("type".into(), Value::String(kind.clone()));
    // Semantic colors require a language server and are deliberately not used.
    obj.remove("semanticTokenColors");
    obj.remove("semanticHighlighting");
    Ok(ImportedTheme {
        id,
        name,
        kind,
        definition: value,
    })
}
