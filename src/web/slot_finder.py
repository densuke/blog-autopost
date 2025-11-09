"""次の空きスロット検索機能。

指定されたSNSの投稿可能タイミングから、次の空きスロットを検索します。
複数SNSへの独立した検索にも対応しています。
"""

from datetime import date, datetime, timedelta
from typing import Dict, List, Optional, Union

from src.timing_manager import TimingManager
from src.web.scheduled_post_store_sqlite import ScheduledPostStoreSQLite
from src.web.timezone_utils import ensure_local_timezone, now_local


class SlotFinder:
    """次の空きスロットを検索するクラス。

    投稿可能タイミング設定に基づいて、指定されたSNSの次の空きスロットを検索します。
    """

    def __init__(
        self,
        timing_manager: TimingManager,
        scheduled_post_store: ScheduledPostStoreSQLite,
        tolerance_minutes: int = 0,
    ):
        """SlotFinderを初期化する。

        Args:
            timing_manager: タイミング管理オブジェクト
            scheduled_post_store: 予約投稿ストア
            tolerance_minutes: 投稿時刻の許容範囲(分)
        """
        self.timing_manager = timing_manager
        self.scheduled_post_store = scheduled_post_store
        self.tolerance_minutes = max(0, tolerance_minutes)

    def generate_candidate_slots(
        self, sns_name: str, start_date: datetime, days: int
    ) -> List[datetime]:
        """候補スロットを時系列順に生成する。

        Args:
            sns_name: SNS名
            start_date: 検索開始日時
            days: 生成日数

        Returns:
            候補スロットのリスト(時系列順)
        """
        allowed_timings = self.timing_manager.get_allowed_timings(sns_name)

        if allowed_timings is None:
            return []

        candidates: List[datetime] = []
        normalized_start = self._normalize_to_local(start_date)
        current_date = normalized_start.date()

        for day_offset in range(days):
            check_date = current_date + timedelta(days=day_offset)
            day_name = self._get_day_name(check_date)

            if day_name not in allowed_timings:
                continue

            for time_str in allowed_timings[day_name]:
                try:
                    hour, minute = int(time_str[:2]), int(time_str[3:5])
                    candidate_dt = datetime(
                        check_date.year, check_date.month, check_date.day, hour, minute
                    )
                    candidate_dt = self._normalize_to_local(candidate_dt)

                    # 過去の時刻はスキップ
                    if candidate_dt <= normalized_start:
                        continue

                    candidates.append(candidate_dt)
                except (ValueError, IndexError):
                    continue

        # 時系列順にソート
        candidates.sort()
        return candidates

    def is_slot_available(self, sns_name: str, slot_time: datetime) -> bool:
        """スロットが空いているかチェックする。

        Args:
            sns_name: SNS名
            slot_time: チェックする日時

        Returns:
            空きがあればTrue
        """
        normalized_slot = self._normalize_to_local(slot_time)

        existing = self.scheduled_post_store.get_posts_by_sns_and_time(
            sns_name,
            normalized_slot,
            tolerance_minutes=self.tolerance_minutes,
        )
        return len(existing) == 0

    def find_next_available_slot(
        self, sns_name: str, start_from: Optional[datetime] = None, max_days: int = 7
    ) -> Optional[datetime]:
        """指定されたSNSの次の空きスロットを検索する。

        Args:
            sns_name: SNS名
            start_from: 検索開始日時(デフォルト: 現在時刻)
            max_days: 最大検索日数

        Returns:
            次の空きスロット。見つからない場合はNone。
        """
        normalized_start_from = self._normalize_to_local(start_from or now_local())

        # 投稿可能タイミング設定を取得
        allowed_timings = self.timing_manager.get_allowed_timings(sns_name)

        if allowed_timings is None:
            return None

        # 候補スロット生成
        candidates = self.generate_candidate_slots(sns_name, normalized_start_from, max_days)

        # 各候補をチェック
        for candidate in candidates:
            if self.is_slot_available(sns_name, candidate):
                return candidate

        # 見つからなかった
        return None

    def find_slots_for_multiple_sns(
        self, sns_list: List[str]
    ) -> Dict[str, Optional[datetime]]:
        """複数SNSの次の空きスロットを一括検索する。

        Args:
            sns_list: SNS名リスト

        Returns:
            SNS名をキー、次の空きスロット(またはNone)を値とする辞書
        """
        result: Dict[str, Optional[datetime]] = {}

        for sns_name in sns_list:
            slot = self.find_next_available_slot(sns_name)
            result[sns_name] = slot

        return result

    @staticmethod
    def _get_day_name(target_date: Union[datetime, date]) -> str:
        """datetimeから曜日名を取得する。

        Args:
            target_date: 日付または日時

        Returns:
            曜日名("Monday", "Tuesday", ...)
        """
        normalized_date = (
            target_date.date() if isinstance(target_date, datetime) else target_date
        )
        if not isinstance(normalized_date, date):
            raise ValueError("target_date must be either date or datetime")
        days = [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ]
        return days[normalized_date.weekday()]

    @staticmethod
    def _normalize_to_local(dt: datetime) -> datetime:
        """naiveなdatetimeをローカルタイムゾーンに揃える。"""
        normalized = ensure_local_timezone(dt)
        if normalized is not None:
            return normalized

        fallback = now_local()
        if dt.tzinfo is None:
            return dt.replace(tzinfo=fallback.tzinfo)
        return fallback
