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
        t.highlight_theme = std::sync::Arc::new(highlight_theme(theme));
    });
}

fn highlight_theme(theme: &ImportedTheme) -> gpui_kit::component::highlighter::HighlightTheme {
    let colors = &theme.definition["colors"];
    let take = |name: &str| colors[name].as_str().and_then(|c| Hsla::parse_hex(c).ok());
    let mut highlight = if theme.kind == "light" {
        (*gpui_kit::component::highlighter::HighlightTheme::default_light()).clone()
    } else {
        (*gpui_kit::component::highlighter::HighlightTheme::default_dark()).clone()
    };
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
                // Contextual selectors require a TextMate scope stack; this
                // native importer intentionally supports simple scopes only.
                if scope.is_empty()
                    || !scope
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
                {
                    continue;
                }
                let key = if scope.starts_with("constant.language.boolean") {
                    "boolean"
                } else if scope.starts_with("string.regexp") {
                    "string.regex"
                } else if scope.starts_with("constant.character.escape") {
                    "string.escape"
                } else if scope.starts_with("keyword.operator") {
                    "operator"
                } else if scope.starts_with("entity.name.tag") {
                    "tag"
                } else if scope.starts_with("entity.other.attribute-name") {
                    "attribute"
                } else if scope.starts_with("meta.property-name")
                    || scope.starts_with("variable.other.property")
                {
                    "property"
                } else if scope.starts_with("punctuation") {
                    "punctuation"
                } else if scope.starts_with("comment") {
                    "comment"
                } else if scope.starts_with("entity.name.function")
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
                } else if scope.starts_with("entity.name.type") || scope.starts_with("support.type")
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
    if let Some(explicit) = theme.definition["nativeSyntax"].as_object() {
        for (name, style) in explicit {
            let mut style = style.clone();
            if let Some(color) = style["color"]
                .as_str()
                .and_then(|c| Hsla::parse_hex(c).ok())
            {
                style["color"] = serde_json::to_value(color).unwrap();
            }
            syntax[name] = style;
        }
    }
    if let Ok(colors) = serde_json::from_value(syntax) {
        highlight.style.syntax = colors;
    }
    highlight
}

#[cfg(test)]
mod tests {
    use super::{builtins, highlight_theme};
    #[test]
    fn every_builtin_has_explicit_detailed_capture_colors() {
        let themes = builtins();
        assert_eq!(themes.len(), 6);
        for theme in themes {
            let highlight = highlight_theme(&theme);
            for capture in [
                "comment",
                "keyword",
                "function",
                "type",
                "property",
                "attribute",
                "operator",
                "string",
                "string.escape",
                "number",
                "boolean",
                "tag",
                "punctuation.bracket",
            ] {
                assert!(
                    highlight
                        .style
                        .syntax
                        .style(capture)
                        .and_then(|s| s.color)
                        .is_some(),
                    "{}: {capture}",
                    theme.id
                );
            }
            assert_ne!(
                highlight.style.syntax.style("comment"),
                highlight.style.syntax.style("string")
            );
        }
    }
    #[test]
    fn theme_a_b_a_and_partial_import_do_not_inherit_previous_tokens() {
        let mut a = builtins().remove(0);
        a.definition = serde_json::json!({"tokenColors":[{"scope":"comment","settings":{"foreground":"#123456"}}]});
        let first = highlight_theme(&a);
        let b = highlight_theme(&builtins().remove(3));
        assert_ne!(first.style.syntax, b.style.syntax);
        let again = highlight_theme(&a);
        assert_eq!(first.style.syntax, again.style.syntax);
        assert_eq!(
            first.style.syntax.string,
            gpui_kit::component::highlighter::HighlightTheme::default_light()
                .style
                .syntax
                .string
        );
    }
    #[test]
    fn unsupported_contextual_and_semantic_theme_rules_are_not_applied() {
        let mut theme = builtins().remove(0);
        theme.definition = serde_json::json!({
            "tokenColors": [
                {"scope":"meta.function entity.name.function", "settings":{"foreground":"#123456"}},
                {"scope":"meta.function>entity.name.function", "settings":{"foreground":"#123456"}},
                {"scope":"comment - comment.block", "settings":{"foreground":"#123456"}}
            ],
            "semanticTokenColors":{"function":"#123456"}
        });
        assert_eq!(
            highlight_theme(&theme).style.syntax,
            gpui_kit::component::highlighter::HighlightTheme::default_light()
                .style
                .syntax
        );
    }
}
