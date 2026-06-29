use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

/// 半角 `#` または全角 `＃` に続くタグ(英数字・下線・各言語の文字)を表す正規表現。
/// `\w` は Unicode モードで日本語などの文字も含む。
static HASHTAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[#＃](\w+)").unwrap());

/// テキスト中のハッシュタグを抽出する(先頭の `#`/`＃` を除いたタグ名のリスト)。
///
/// - 出現順を保ちつつ重複を除外する。
/// - 数字のみのタグ(例: `#1`)は章番号等の誤検出を避けるため除外する。
pub fn extract_hashtags(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for cap in HASHTAG_RE.captures_iter(text) {
        let tag = cap[1].to_string();
        if tag.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if seen.insert(tag.clone()) {
            out.push(tag);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_basic() {
        let tags = extract_hashtags("動画です #Rust #プログラミング よろしく");
        assert_eq!(tags, vec!["Rust".to_string(), "プログラミング".to_string()]);
    }

    #[test]
    fn test_extract_dedup_and_order() {
        let tags = extract_hashtags("#a #b #a #c");
        assert_eq!(
            tags,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn test_extract_fullwidth_hash() {
        let tags = extract_hashtags("全角ハッシュ ＃技術 も拾う");
        assert_eq!(tags, vec!["技術".to_string()]);
    }

    #[test]
    fn test_extract_skips_numeric_only() {
        // 数字のみのタグ(#1)は除外、英数字混在(#news2)は残す
        let tags = extract_hashtags("番号は #1 です #news2");
        assert_eq!(tags, vec!["news2".to_string()]);
    }

    #[test]
    fn test_extract_none() {
        assert!(extract_hashtags("タグなしのテキスト").is_empty());
    }
}
