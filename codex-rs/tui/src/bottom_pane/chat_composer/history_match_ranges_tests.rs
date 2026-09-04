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
