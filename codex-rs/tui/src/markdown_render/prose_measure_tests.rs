use super::*;
use pretty_assertions::assert_eq;

#[test]
fn prose_reflows_to_viewport_width_with_words_links_and_code_preserved() {
    let paragraph =
        "A careful explanation keeps its source context while a reader follows the result. "
            .repeat(4);
    let linked = format!(
        "{paragraph}[Read the complete release notes](https://example.test/releases). 日本語 remains intact."
    );
    let lines = render_markdown_lines_with_width_and_cwd(&linked, Some(120), /*cwd*/ None);
    assert!(lines.iter().all(|line| line.line.width() <= 120));
    assert!(
        lines
            .iter()
            .flat_map(|line| &line.hyperlinks)
            .any(|link| link.destination == "https://example.test/releases")
    );
    assert!(lines.iter().all(|line| {
        line.hyperlinks
            .iter()
            .all(|link| link.columns.end <= line.line.width())
    }));
    let narrow = render_markdown_text_with_width(&paragraph, Some(48));
    let wide = render_markdown_text_with_width(&paragraph, Some(120));
    assert_eq!(
        narrow.to_string().split_whitespace().collect::<Vec<_>>(),
        wide.to_string().split_whitespace().collect::<Vec<_>>(),
    );
    insta::assert_snapshot!(
        "prose_follows_viewport_width",
        format!("48 columns\n{narrow}\n\n120 columns\n{wide}")
    );

    let code = format!("let original_spacing = {};", "1 + ".repeat(30) + "2");
    let rendered = render_markdown_text_with_width(&format!("```rust\n{code}\n```"), Some(140));
    assert_eq!(rendered.to_string(), code);
    let intrinsic = render_markdown_text(&paragraph);
    assert!(intrinsic.lines[0].width() > 120);
}
