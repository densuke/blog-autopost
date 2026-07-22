//! `--sns` オプションの指定を解釈し、対象SNSを判定する。
//!
//! `check` と `post` の双方で同じ書式を扱うため、判定ロジックをここへ集約する。

use std::collections::HashSet;

/// `--sns` の指定を解釈した結果。
///
/// 書式はカンマ区切りで、以下を受け付ける。
///
/// - `mastodon` — SNS種別またはアカウント名での指定
/// - `-x` — 先頭の `-` で除外
/// - `all` — 全件を対象にする
///
/// 指定が無い場合、および除外のみが指定された場合は全件が対象となる。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SnsSelector {
    included: HashSet<String>,
    excluded: HashSet<String>,
    has_all: bool,
    /// 対象の明示指定が無く、暗黙的に全件が対象となる状態か。
    implicit_all: bool,
}

impl SnsSelector {
    /// `--sns` の値を解釈する。`None` は全件対象を意味する。
    pub fn parse(spec: Option<&str>) -> Self {
        let mut included = HashSet::new();
        let mut excluded = HashSet::new();
        let mut has_all = false;

        if let Some(spec) = spec {
            for part in spec.split(',') {
                let part = part.trim().to_lowercase();
                if part.is_empty() {
                    continue;
                }
                if let Some(name) = part.strip_prefix('-') {
                    excluded.insert(name.to_string());
                } else if part == "all" {
                    has_all = true;
                } else {
                    included.insert(part);
                }
            }
        }

        let implicit_all = spec.is_none() || (included.is_empty() && !has_all);

        Self {
            included,
            excluded,
            has_all,
            implicit_all,
        }
    }

    /// SNS種別とアカウント名の組が対象に含まれるか判定する。
    ///
    /// 除外指定は対象指定より優先される。
    pub fn matches(&self, kind: &str, account_name: &str) -> bool {
        let kind = kind.to_lowercase();
        let account = account_name.to_lowercase();

        let is_targeted = self.implicit_all
            || self.has_all
            || self.included.contains(&kind)
            || self.included.contains(&account);
        let is_excluded = self.excluded.contains(&kind) || self.excluded.contains(&account);

        is_targeted && !is_excluded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_targets_everything() {
        let sel = SnsSelector::parse(None);

        assert!(sel.matches("mastodon", "mstdn-main"));
        assert!(sel.matches("x", "x-main"));
    }

    #[test]
    fn test_empty_string_targets_everything() {
        let sel = SnsSelector::parse(Some(""));

        assert!(sel.matches("mastodon", "mstdn-main"));
    }

    #[test]
    fn test_select_by_kind() {
        let sel = SnsSelector::parse(Some("mastodon"));

        assert!(sel.matches("mastodon", "mstdn-main"));
        assert!(!sel.matches("misskey", "misskey-main"));
    }

    #[test]
    fn test_select_by_account_name() {
        let sel = SnsSelector::parse(Some("mstdn-main"));

        assert!(sel.matches("mastodon", "mstdn-main"));
        assert!(!sel.matches("mastodon", "mstdn-sub"));
    }

    #[test]
    fn test_select_multiple() {
        let sel = SnsSelector::parse(Some("mastodon,bluesky"));

        assert!(sel.matches("mastodon", "a"));
        assert!(sel.matches("bluesky", "b"));
        assert!(!sel.matches("misskey", "c"));
    }

    /// 除外のみの指定では、除外されたもの以外がすべて対象になる。
    #[test]
    fn test_exclude_only() {
        let sel = SnsSelector::parse(Some("-x"));

        assert!(!sel.matches("x", "x-main"));
        assert!(sel.matches("mastodon", "mstdn-main"));
    }

    /// 除外は対象指定より優先される。
    #[test]
    fn test_exclude_wins_over_include() {
        let sel = SnsSelector::parse(Some("mastodon,-mastodon"));

        assert!(!sel.matches("mastodon", "mstdn-main"));
    }

    #[test]
    fn test_all_keyword() {
        let sel = SnsSelector::parse(Some("all"));

        assert!(sel.matches("mastodon", "a"));
        assert!(sel.matches("x", "b"));
    }

    #[test]
    fn test_all_keyword_with_exclusion() {
        let sel = SnsSelector::parse(Some("all,-x"));

        assert!(sel.matches("mastodon", "a"));
        assert!(!sel.matches("x", "b"));
    }

    #[test]
    fn test_case_insensitive() {
        let sel = SnsSelector::parse(Some("MASTODON"));

        assert!(sel.matches("mastodon", "a"));
        assert!(sel.matches("Mastodon", "A"));
    }

    /// 空要素や余分な空白は無視される。
    #[test]
    fn test_ignores_blank_parts() {
        let sel = SnsSelector::parse(Some(" mastodon , , bluesky "));

        assert!(sel.matches("mastodon", "a"));
        assert!(sel.matches("bluesky", "b"));
        assert!(!sel.matches("misskey", "c"));
    }
}
