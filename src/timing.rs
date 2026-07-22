use crate::config::Config;
use crate::scheduled::store::JsonScheduledPostStore;
use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, Weekday};
use std::collections::{BTreeSet, HashMap};

pub struct TimingManager {
    // SNS設定名 -> (曜日 -> 許可された時刻のソート済みセット)
    sns_timings: HashMap<String, HashMap<Weekday, BTreeSet<NaiveTime>>>,
}

impl TimingManager {
    pub fn new(config: &Config) -> Self {
        let mut sns_timings = HashMap::new();

        // config からグローバル設定を取得
        let global_raw = config.default_allowed_timings.clone().unwrap_or_default();

        // 1. 各 SNS 向けに設定をマージする
        if let Some(allowed_map) = &config.allowed_timings {
            for (sns_name, spec_list) in allowed_map {
                let mut merged = HashMap::new();

                // まずグローバル設定を展開
                Self::merge_spec_list(&mut merged, &global_raw);

                // 次にSNS固有の設定を展開してマージ (上書きまたは追加)
                Self::merge_spec_list(&mut merged, spec_list);

                sns_timings.insert(sns_name.clone(), merged);
            }
        }

        // 2. 固有設定がないSNSが config に定義されている場合、グローバル設定のみを適用
        for sns_conf in &config.sns {
            let sns_name = match sns_conf {
                crate::config::SnsConfig::Mastodon { name, .. } => name,
                crate::config::SnsConfig::Misskey { name, .. } => name,
                crate::config::SnsConfig::Bluesky { name, .. } => name,
                crate::config::SnsConfig::X { name, .. } => name,
                crate::config::SnsConfig::Threads { name, .. } => name,
                crate::config::SnsConfig::Tumblr { name, .. } => name,
                _ => continue,
            };

            if !sns_timings.contains_key(sns_name) {
                let mut merged = HashMap::new();
                Self::merge_spec_list(&mut merged, &global_raw);
                sns_timings.insert(sns_name.clone(), merged);
            }
        }

        Self { sns_timings }
    }

    /// 投稿可能タイミングが定義されているかを返す（制限なしモード判定用）
    pub fn has_timings(&self, sns_name: &str) -> bool {
        if let Some(timings) = self.sns_timings.get(sns_name) {
            !timings.is_empty()
        } else {
            false
        }
    }

    /// 指定されたSNSの指定された曜日に許可された時刻リストを取得
    pub fn get_allowed_times(&self, sns_name: &str, weekday: Weekday) -> Vec<NaiveTime> {
        if let Some(timings) = self.sns_timings.get(sns_name)
            && let Some(times) = timings.get(&weekday)
        {
            return times.iter().cloned().collect();
        }
        Vec::new()
    }

    // 仕様リストを展開してマップにマージする
    fn merge_spec_list(
        map: &mut HashMap<Weekday, BTreeSet<NaiveTime>>,
        spec_list: &[(String, Vec<String>)],
    ) {
        for (day_spec, times_raw) in spec_list {
            let weekdays = Self::expand_wildcard(day_spec);
            let times: Vec<NaiveTime> = times_raw
                .iter()
                .filter_map(|t| NaiveTime::parse_from_str(t, "%H:%M").ok())
                .collect();

            for day in weekdays {
                map.entry(day).or_default().extend(times.clone());
            }
        }
    }

    // 曜日指定文字列をWeekdayのリストに展開
    fn expand_wildcard(day_spec: &str) -> Vec<Weekday> {
        match day_spec {
            "*" => vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
                Weekday::Sat,
                Weekday::Sun,
            ],
            "Weekday" => vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ],
            "Weekend" => vec![Weekday::Sat, Weekday::Sun],
            other => {
                if let Some(w) = Self::parse_weekday(other) {
                    vec![w]
                } else {
                    vec![]
                }
            }
        }
    }

    fn parse_weekday(s: &str) -> Option<Weekday> {
        match s.to_lowercase().as_str() {
            "monday" | "mon" => Some(Weekday::Mon),
            "tuesday" | "tue" => Some(Weekday::Tue),
            "wednesday" | "wed" => Some(Weekday::Wed),
            "thursday" | "thu" => Some(Weekday::Thu),
            "friday" | "fri" => Some(Weekday::Fri),
            "saturday" | "sat" => Some(Weekday::Sat),
            "sunday" | "sun" => Some(Weekday::Sun),
            _ => None,
        }
    }
}

pub struct SlotFinder<'a> {
    timing_manager: &'a TimingManager,
    store: &'a JsonScheduledPostStore,
    tolerance_minutes: i64,
}

impl<'a> SlotFinder<'a> {
    pub fn new(
        timing_manager: &'a TimingManager,
        store: &'a JsonScheduledPostStore,
        tolerance_minutes: i64,
    ) -> Self {
        Self {
            timing_manager,
            store,
            tolerance_minutes: tolerance_minutes.max(0),
        }
    }

    /// 指定されたSNSの次の空きスロット（投稿予定日時）を検索する
    pub async fn find_next_available_slot(
        &self,
        sns_name: &str,
        start_from: Option<DateTime<Local>>,
        max_days: i64,
    ) -> Result<Option<DateTime<Local>>> {
        let base_start = start_from.unwrap_or_else(Local::now);

        // 制限なしモード：タイミング設定がない場合は、現在（or開始）時刻の1分後（許容範囲+1分後）を返す
        if !self.timing_manager.has_timings(sns_name) {
            let offset = (self.tolerance_minutes + 1).max(1);
            return Ok(Some(base_start + Duration::minutes(offset)));
        }

        // 候補日時を時系列順に生成してチェック
        let candidates = self.generate_candidate_slots(sns_name, base_start, max_days);

        for candidate in candidates {
            if self.is_slot_available(sns_name, candidate).await? {
                return Ok(Some(candidate));
            }
        }

        Ok(None)
    }

    // 候補日時のリストを生成する
    fn generate_candidate_slots(
        &self,
        sns_name: &str,
        start_time: DateTime<Local>,
        days: i64,
    ) -> Vec<DateTime<Local>> {
        let mut candidates = Vec::new();
        let min_time = start_time + Duration::minutes(self.tolerance_minutes);

        for day_offset in 0..days {
            let check_date = (start_time + Duration::days(day_offset)).date_naive();
            let weekday = check_date.weekday();

            let allowed_times = self.timing_manager.get_allowed_times(sns_name, weekday);
            for time in allowed_times {
                if let Some(candidate_dt) =
                    check_date.and_time(time).and_local_timezone(Local).single()
                    && candidate_dt > min_time
                {
                    candidates.push(candidate_dt);
                }
            }
        }

        candidates.sort();
        candidates
    }

    // 指定されたスロットが他の予約で埋まっていないかチェックする
    async fn is_slot_available(&self, sns_name: &str, slot_time: DateTime<Local>) -> Result<bool> {
        let existing = self
            .store
            .get_posts_by_sns_and_time(sns_name, slot_time, self.tolerance_minutes)
            .await?;

        Ok(existing.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, SnsConfig};
    use crate::scheduled::models::ScheduledPost;
    use chrono::{NaiveDate, Timelike};
    use tempfile::NamedTempFile;

    fn test_config() -> Config {
        Config {
            announcement_text: None,
            blog: None,
            sns: vec![SnsConfig::Mastodon {
                name: "mstdn-main".to_string(),
                instance_url: "https://mstdn.example.com".to_string(),
                access_token: "dummy".to_string(),
            }],
            templates: HashMap::new(),
            default_allowed_timings: Some(vec![(
                "*".to_string(),
                vec!["09:00".to_string(), "18:00".to_string()],
            )]),
            allowed_timings_tolerance_minutes: Some(5),
            allowed_timings: Some({
                let mut map = HashMap::new();
                map.insert(
                    "mstdn-main".to_string(),
                    vec![("Weekday".to_string(), vec!["12:00".to_string()])],
                );
                map
            }),
            web_auth: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn test_timing_manager_merge() {
        let config = test_config();
        let manager = TimingManager::new(&config);

        // mstdn-main の月曜日(Weekday)のタイミング：
        // グローバル("09:00", "18:00") + 固有("12:00") = 3個
        let times = manager.get_allowed_times("mstdn-main", Weekday::Mon);
        assert_eq!(times.len(), 3);
        assert_eq!(times[0], NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        assert_eq!(times[1], NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        assert_eq!(times[2], NaiveTime::from_hms_opt(18, 0, 0).unwrap());

        // 土曜日(Weekend)のタイミング：
        // グローバル("09:00", "18:00") のみ (固有は Weekday のみなので)
        let times_sat = manager.get_allowed_times("mstdn-main", Weekday::Sat);
        assert_eq!(times_sat.len(), 2);
        assert_eq!(times_sat[0], NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        assert_eq!(times_sat[1], NaiveTime::from_hms_opt(18, 0, 0).unwrap());
    }

    #[tokio::test]
    async fn test_slot_finder() {
        let config = test_config();
        let manager = TimingManager::new(&config);

        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonScheduledPostStore::new(temp_file.path());

        let finder = SlotFinder::new(&manager, &store, 5);

        // 2026年6月22日(月) 10:00:00 から検索開始
        let start_time = NaiveDate::from_ymd_opt(2026, 6, 22)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap();

        // 候補の次のスロットは、月曜日(本日)の 12:00
        let slot = finder
            .find_next_available_slot("mstdn-main", Some(start_time), 7)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(slot.time().hour(), 12);
        assert_eq!(slot.time().minute(), 0);
        assert_eq!(slot.date_naive(), start_time.date_naive());

        // 12:00 に予約を1件追加する
        let post = ScheduledPost::new(
            "予約".to_string(),
            slot,
            vec![],
            vec!["mstdn-main".to_string()],
        );
        store.create_post(post).await.unwrap();

        // 再度 10:00 から検索すると、12:00 は埋まっているので次の候補である 18:00 が返るべき
        let slot2 = finder
            .find_next_available_slot("mstdn-main", Some(start_time), 7)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(slot2.time().hour(), 18);
        assert_eq!(slot2.time().minute(), 0);
    }

    /// 曜日指定の展開を検証する。
    #[test]
    fn test_expand_wildcard() {
        assert_eq!(TimingManager::expand_wildcard("*").len(), 7);
        assert_eq!(
            TimingManager::expand_wildcard("Weekday"),
            vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri
            ]
        );
        assert_eq!(
            TimingManager::expand_wildcard("Weekend"),
            vec![Weekday::Sat, Weekday::Sun]
        );
        assert_eq!(TimingManager::expand_wildcard("mon"), vec![Weekday::Mon]);
        assert!(TimingManager::expand_wildcard("知らない曜日").is_empty());
    }

    /// 曜日名は大文字小文字と省略形の双方を受け付ける。
    #[test]
    fn test_parse_weekday() {
        assert_eq!(TimingManager::parse_weekday("Monday"), Some(Weekday::Mon));
        assert_eq!(TimingManager::parse_weekday("MON"), Some(Weekday::Mon));
        assert_eq!(TimingManager::parse_weekday("sunday"), Some(Weekday::Sun));
        assert_eq!(TimingManager::parse_weekday("sat"), Some(Weekday::Sat));
        assert_eq!(TimingManager::parse_weekday("nope"), None);
    }

    /// タイミング未設定のSNSでは has_timings が偽になる。
    #[test]
    fn test_has_timings() {
        let manager = TimingManager::new(&test_config());

        assert!(manager.has_timings("mstdn-main"));
        assert!(!manager.has_timings("設定されていないSNS"));
    }

    /// 未設定のSNSに対する許可時刻は空になる。
    #[test]
    fn test_get_allowed_times_for_unknown_sns() {
        let manager = TimingManager::new(&test_config());

        assert!(
            manager
                .get_allowed_times("設定されていないSNS", Weekday::Mon)
                .is_empty()
        );
    }

    /// 解釈できない時刻表記は無視される。
    #[test]
    fn test_invalid_time_format_is_ignored() {
        let mut config = test_config();
        // SNS固有設定を外し、既定の指定だけが効くようにする
        config.allowed_timings = None;
        config.default_allowed_timings = Some(vec![(
            "*".to_string(),
            vec!["09:00".to_string(), "25:99".to_string(), "あさ".to_string()],
        )]);

        let manager = TimingManager::new(&config);
        let times = manager.get_allowed_times("mstdn-main", Weekday::Mon);

        assert_eq!(times.len(), 1, "不正な表記は無視されるはず: {:?}", times);
        assert_eq!(times[0].hour(), 9);
    }

    /// 曜日別の指定が既定の指定へ追加される。
    #[test]
    fn test_per_sns_timings_are_merged() {
        let mut config = test_config();
        let mut per_sns = HashMap::new();
        per_sns.insert(
            "mstdn-main".to_string(),
            vec![("Weekend".to_string(), vec!["12:00".to_string()])],
        );
        config.allowed_timings = Some(per_sns);

        let manager = TimingManager::new(&config);

        let saturday = manager.get_allowed_times("mstdn-main", Weekday::Sat);
        assert!(
            saturday.iter().any(|t| t.hour() == 12),
            "土曜には12:00が含まれるはず: {:?}",
            saturday
        );
    }

    /// タイミング未設定のSNSは「制限なしモード」として扱われ、
    /// 許容範囲の直後の時刻が枠として返る。
    #[tokio::test]
    async fn test_find_next_slot_without_timings_uses_unrestricted_mode() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonScheduledPostStore::new(temp_file.path());
        let manager = TimingManager::new(&test_config());
        let tolerance = 5;
        let finder = SlotFinder::new(&manager, &store, tolerance);

        let base = Local::now();
        let slot = finder
            .find_next_available_slot("設定されていないSNS", Some(base), 7)
            .await
            .expect("エラーにはならない")
            .expect("制限なしモードでは必ず枠が返る");

        assert_eq!(slot, base + Duration::minutes(tolerance + 1));
    }

    /// 探索日数が0なら枠は見つからない。
    #[tokio::test]
    async fn test_find_next_slot_with_zero_days() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonScheduledPostStore::new(temp_file.path());
        let manager = TimingManager::new(&test_config());
        let finder = SlotFinder::new(&manager, &store, 5);

        let slot = finder
            .find_next_available_slot("mstdn-main", None, 0)
            .await
            .unwrap();

        assert!(slot.is_none());
    }

    /// 既存の予約と重なる枠は避けて次の枠が選ばれる。
    #[tokio::test]
    async fn test_find_next_slot_avoids_occupied() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonScheduledPostStore::new(temp_file.path());
        let manager = TimingManager::new(&test_config());
        let finder = SlotFinder::new(&manager, &store, 5);

        // まず1つ目の枠を取得して埋める
        let first = finder
            .find_next_available_slot("mstdn-main", None, 7)
            .await
            .unwrap()
            .expect("枠が見つかるはず");

        store
            .create_post(ScheduledPost::new(
                "先客".to_string(),
                first,
                vec![],
                vec!["mstdn-main".to_string()],
            ))
            .await
            .unwrap();

        let second = finder
            .find_next_available_slot("mstdn-main", None, 7)
            .await
            .unwrap()
            .expect("次の枠が見つかるはず");

        assert_ne!(first, second, "埋まった枠は避けるはず");
        assert!(second > first);
    }
}
