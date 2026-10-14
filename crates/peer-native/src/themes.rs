use gpui_kit::{
    component::{Colorize, Theme},
    *,
};
use peer_core::ImportedTheme;

pub fn builtins() -> Vec<ImportedTheme> {
    [
        (
            "github-light",
            include_str!("../../../assets/themes/github-light.json"),
        ),
        (
            "github-dark",
            include_str!("../../../assets/themes/github-dark.json"),
        ),
        (
            "one-dark-pro",
            include_str!("../../../assets/themes/one-dark-pro.json"),
        ),
        (
            "dracula",
            include_str!("../../../assets/themes/dracula.json"),
        ),
        (
            "catppuccin-latte",
            include_str!("../../../assets/themes/catppuccin-latte.json"),
        ),
        (
            "catppuccin-mocha",
            include_str!("../../../assets/themes/catppuccin-mocha.json"),
        ),
    ]
    .into_iter()
    .filter_map(|(id, text)| {
        serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .map(|definition| ImportedTheme {
                id: id.into(),
                name: definition["displayName"].as_str().unwrap_or(id).into(),
                kind: definition["type"].as_str().unwrap_or("dark").into(),
                definition,
            })
    })
    .collect()
}
pub fn apply(theme: &ImportedTheme, font: u32, cx: &mut App) {
    let colors = &theme.definition["colors"];
    let take = |name: &str| colors[name].as_str().and_then(|c| Hsla::parse_hex(c).ok());
    Theme::update(cx, |t| {
        t.mono_font_size = px(font as f32);
        macro_rules! color {
            ($field:ident,$key:literal) => {
                if let Some(value) = take($key) {
                    t.colors.$field = value;
                }
            };
        }
        color!(background, "editor.background");
        color!(foreground, "editor.foreground");
        color!(border, "panel.border");
        color!(selection, "editor.selectionBackground");
        color!(primary, "button.background");
        color!(primary_foreground, "button.foreground");
        color!(muted_foreground, "descriptionForeground");
        color!(danger, "gitDecoration.deletedResourceForeground");
        color!(success, "gitDecoration.addedResourceForeground");
        let mut highlight = (*t.highlight_theme).clone();
        highlight.style.editor_background = take("editor.background");
        highlight.style.editor_foreground = take("editor.foreground");
        highlight.style.editor_line_number = take("editorLineNumber.foreground");
        // Translate TextMate token categories to the native Tree-sitter theme categories.
        let mut syntax = serde_json::to_value(&highlight.style.syntax).unwrap_or_default();
        if let Some(rules) = theme.definition["tokenColors"].as_array() {
            for rule in rules {
                let scopes: Vec<&str> = if let Some(s) = rule["scope"].as_str() {
                    s.split(',').map(str::trim).collect()
                } else {
                    rule["scope"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                        .unwrap_or_default()
                };
                let fg = rule["settings"]["foreground"]
                    .as_str()
                    .and_then(|c| Hsla::parse_hex(c).ok());
                let font = rule["settings"]["fontStyle"].as_str();
                if fg.is_none() && font.is_none() {
                    continue;
                }
                for scope in scopes {
                    let key = if scope.starts_with("comment") {
                        "comment"
                    } else if scope.contains("entity.name.function")
                        || scope.starts_with("support.function")
                    {
                        "function"
                    } else if scope.starts_with("keyword") || scope.starts_with("storage") {
                        "keyword"
                    } else if scope.starts_with("string") {
                        "string"
                    } else if scope.starts_with("constant.numeric") {
                        "number"
                    } else if scope.starts_with("constant") {
                        "constant"
                    } else if scope.contains("entity.name.type")
                        || scope.starts_with("support.type")
                    {
                        "type"
                    } else if scope.starts_with("variable") {
                        "variable"
                    } else {
                        continue;
                    };
                    let mut style = syntax[key].as_object().cloned().unwrap_or_default();
                    if let Some(fg) = fg {
                        style.insert("color".into(), serde_json::to_value(fg).unwrap());
                    }
                    if let Some(font) = font {
                        style.insert(
                            "font_style".into(),
                            if font.split_whitespace().any(|s| s == "italic") {
                                "italic"
                            } else {
                                "normal"
                            }
                            .into(),
                        );
                        style.insert(
                            "font_weight".into(),
                            if font.split_whitespace().any(|s| s == "bold") {
                                700
                            } else {
                                400
                            }
                            .into(),
                        );
                    }
                    syntax[key] = style.into();
                }
            }
        }
        if let Ok(colors) = serde_json::from_value(syntax) {
            highlight.style.syntax = colors;
        }
        t.highlight_theme = std::sync::Arc::new(highlight);
    });
}
