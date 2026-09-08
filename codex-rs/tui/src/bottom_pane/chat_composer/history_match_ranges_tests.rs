use super::case_insensitive_match_ranges;
use pretty_assertions::assert_eq;

#[test]
fn history_search_match_ranges_are_case_insensitive() {
    assert_eq!(
        case_insensitive_match_ranges("git status git", "GIT"),
        vec![0..3, 11..14]
    );
    assert_eq!(case_insensitive_match_ranges("aİ i", "i"), vec![1..3, 4..5]);
    assert!(case_insensitive_match_ranges("git", "").is_empty());
}

#[test]
fn history_search_match_ranges_preserve_unicode_boundaries() {
    for (text, query, expected) in [
        ("İİ", "i", vec![0..2, 2..4]),
        ("İİ", "\u{307}", vec![0..2, 2..4]),
        ("İİİ", "\u{307}i", vec![0..4, 2..6]),
        ("éÉ é", "É", vec![0..2, 2..4, 5..7]),
        ("aaaaa", "aa", vec![0..2, 2..4]),
        ("", "x", vec![]),
        ("abc", "z", vec![]),
    ] {
        assert_eq!(
            case_insensitive_match_ranges(text, query),
            expected,
            "text: {text:?}, query: {query:?}"
        );
    }
}
