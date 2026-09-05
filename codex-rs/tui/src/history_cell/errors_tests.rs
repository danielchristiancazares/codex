use super::*;
use pretty_assertions::assert_eq;

#[test]
fn error_prose_reflows_without_changing_raw_copy() {
    let message = "The connection was interrupted after several retries. Your draft is preserved. Check the connection and try again when it is ready.";
    let cell = ErrorHistoryCell { message: message.to_string() };
    let raw = vec![Line::from(format!("■ {message}"))];
    let mut frames = Vec::new();
    for width in [18, 48, 80, 120] {
        let lines = cell.display_lines(width);
        assert!(lines.iter().all(|line| line.width() <= usize::from(width.min(88))));
        assert_eq!(cell.raw_lines(), raw);
        frames.push(format!("{width} columns\n{}", lines.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")));
    }
    insta::assert_snapshot!("error_prose_measure", frames.join("\n\n"));
}
