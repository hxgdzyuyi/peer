use gpui_kit::component::{highlighter::SyntaxHighlighter, Rope};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

fn captures(language: &str, text: &str) -> Vec<(std::ops::Range<usize>, gpui_kit::SharedString)> {
    SyntaxHighlighter::new(language).capture_names_with_budget(
        &Rope::from_str(text),
        Duration::from_secs(2),
        &AtomicBool::new(false),
    )
}
fn covers(
    captures: &[(std::ops::Range<usize>, gpui_kit::SharedString)],
    source: &str,
    needle: &str,
    category: &str,
) -> bool {
    let start = source.find(needle).unwrap();
    captures
        .iter()
        .any(|(r, n)| r.start <= start && r.end >= start + needle.len() && n.starts_with(category))
}
#[test]
fn formerly_empty_queries_capture_real_tokens() {
    for (language, source, needle) in [
        ("swift", "func greet() { print(\"hello\") }", "hello"),
        (
            "csharp",
            "class Greeter { string Message = \"hello\"; }",
            "hello",
        ),
        ("graphql", "query { user(name: \"hello\") { id } }", "hello"),
        (
            "proto",
            "syntax = \"proto3\"; message Hello { string name = 1; }",
            "proto3",
        ),
        ("cmake", "set(MESSAGE \"hello\")", "hello"),
    ] {
        let result = captures(language, source);
        assert!(!result.is_empty(), "{language}");
        assert!(
            covers(&result, source, needle, "string"),
            "{language}: {result:?}"
        );
    }
}
#[test]
fn html_and_markdown_use_bounded_injected_languages() {
    let html = "<script>const answer = 42;</script><style>.card { color: red; }</style>";
    let result = captures("html", html);
    assert!(covers(&result, html, "42", "number"), "{result:?}");
    assert!(covers(&result, html, "color", "property"), "{result:?}");
    let md = "```rust\nfn greet() { let x = 123; }\n```\n\n```html\n<script>const answer = 456;</script>\n```\n";
    let result = captures("markdown", md);
    assert!(covers(&result, md, "greet", "function"), "{result:?}");
    assert!(covers(&result, md, "456", "number"), "{result:?}");
}
#[test]
fn injection_count_is_bounded_and_host_text_remains_available() {
    let source = (0..40)
        .map(|i| format!("```rust\nfn function_{i}() {{}}\n```\n\n"))
        .collect::<String>();
    let result = captures("markdown", &source);
    assert!(covers(&result, &source, "function_0", "function"));
    assert!(!covers(&result, &source, "function_39", "function"));
    assert!(result.iter().all(|(r, _)| r.end <= source.len()));
}
#[test]
fn cancelled_and_exhausted_work_is_plain_text_and_next_pass_recovers() {
    let mut parser = SyntaxHighlighter::new("javascript");
    let text = Rope::from_str("const answer = 42;");
    assert!(parser
        .capture_names_with_budget(&text, Duration::from_secs(1), &AtomicBool::new(true))
        .is_empty());
    assert!(parser
        .capture_names_with_budget(&text, Duration::ZERO, &AtomicBool::new(false))
        .is_empty());
    let large = Rope::from_str(&"const item = { x: 42 };\n".repeat(60_000));
    let start = Instant::now();
    assert!(parser
        .capture_names_with_budget(&large, Duration::from_millis(1), &AtomicBool::new(false))
        .is_empty());
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "bounded parse/query did not return"
    );
    assert!(!parser
        .capture_names_with_budget(&text, Duration::from_secs(1), &AtomicBool::new(false))
        .is_empty());
}

#[test]
fn nested_markdown_does_not_parse_beyond_two_injection_levels() {
    let source = "````markdown\n```html\n<script>const answer = 987;</script>\n```\n````\n";
    let result = captures("markdown", source);
    assert!(!result.is_empty());
    assert!(!covers(&result, source, "987", "number"));
}

#[test]
fn unrelated_files_do_not_reuse_an_unterminated_comment_tree() {
    let mut parser = SyntaxHighlighter::new("rust");
    parser.capture_names_with_budget(
        &Rope::from_str("/* unfinished"),
        Duration::from_secs(1),
        &AtomicBool::new(false),
    );
    let source = "fn clean_file() {}";
    let result = parser.capture_names_with_budget(
        &Rope::from_str(source),
        Duration::from_secs(1),
        &AtomicBool::new(false),
    );
    assert!(covers(&result, source, "clean_file", "function"));
    assert!(!covers(&result, source, "clean_file", "comment"));
}

#[test]
fn cancellation_interrupts_in_flight_capture_work() {
    use std::sync::atomic::Ordering;
    let mut parser = SyntaxHighlighter::new("javascript");
    let source = Rope::from_str(&"const item = { x: 42 };\n".repeat(60_000));
    let cancelled = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(1));
            cancelled.store(true, Ordering::Relaxed);
        });
        parser.capture_names_with_budget(&source, Duration::from_secs(5), &cancelled)
    });
    assert!(
        result.is_empty(),
        "Cancelled work must not publish partial captures"
    );
}

#[test]
fn source_and_capture_count_limits_degrade_to_plain_text() {
    let mut parser = SyntaxHighlighter::new("json");
    let cancelled = AtomicBool::new(false);
    let oversized = Rope::from_str(&" ".repeat(2 * 1024 * 1024 + 1));
    assert!(parser
        .capture_names_with_budget(&oversized, Duration::from_secs(5), &cancelled)
        .is_empty());
    let many_captures = Rope::from_str(&format!("[{}0]", "1,".repeat(100_001)));
    assert!(parser
        .capture_names_with_budget(&many_captures, Duration::from_secs(5), &cancelled)
        .is_empty());
    assert!(!parser
        .capture_names_with_budget(&Rope::from_str("[42]"), Duration::from_secs(1), &cancelled)
        .is_empty());
}

#[test]
fn cold_injected_query_initialization_does_not_consume_execution_budget() {
    let source = "```swift\nfunc greet() { print(\"cold query\") }\n```\n";
    let mut parser = SyntaxHighlighter::new("markdown");
    let result = parser.capture_names_with_budget(
        &Rope::from_str(source),
        Duration::from_millis(100),
        &AtomicBool::new(false),
    );
    assert!(
        covers(&result, source, "cold query", "string"),
        "{result:?}"
    );
}
