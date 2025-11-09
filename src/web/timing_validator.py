"""予約投稿実行時のタイミング検証モジュール。

実行時刻が投稿可能タイミングの範囲内かをチェックします。
許容範囲(±N分)を考慮した柔軟な検証機能を提供します。
"""

from datetime import datetime
from typing import Optional, Tuple

from src.timing_manager import TimingManager


class TimingValidator:
    """予約投稿実行時のタイミング検証。

    実行時刻が投稿可能タイミングの範囲内かを検証し、
    許容範囲を考慮した判定を行います。
    """

    def __init__(
        self,
        timing_manager: TimingManager,
        tolerance_minutes: int = 5
    ):
        """TimingValidatorを初期化する。

        Args:
            timing_manager: タイミング管理オブジェクト
            tolerance_minutes: 許容範囲(分)。デフォルト: 5分
        """
        self.timing_manager = timing_manager
        self.tolerance_minutes = tolerance_minutes

    def validate_timing(
        self,
        sns_name: str,
        execution_time: datetime
    ) -> Tuple[bool, Optional[str]]:
        """実行時刻が許可範囲内かチェックする。

        Args:
            sns_name: SNS名
            execution_time: 実行時刻

        Returns:
            (valid, reason)のタプル
            - valid: 許可範囲内ならTrue
            - reason: Falseの場合のスキップ理由
        """
        # 投稿可能タイミング設定を取得
        allowed_timings = self.timing_manager.get_allowed_timings(sns_name)

        # 設定なし(制限なし)の場合は常に許可
        if allowed_timings is None:
            return (True, None)

        # 実行時刻の曜日を取得
        day_of_week = self.get_day_of_week(execution_time)

        # 該当曜日の時刻リストを取得
        if day_of_week not in allowed_timings:
            reason = f"投稿時刻が許可されたタイミングの範囲外のため実行をスキップしました(曜日: {day_of_week})"
            return (False, reason)

        times = allowed_timings[day_of_week]

        # 各時刻をチェック
        for target_time in times:
            if self.is_time_within_tolerance(target_time, execution_time, self.tolerance_minutes):
                return (True, None)

        # すべての時刻が範囲外
        reason = f"投稿時刻が許可されたタイミングの範囲外のため実行をスキップしました(実行時刻: {execution_time.strftime('%H:%M')})"
        return (False, reason)

    def is_time_within_tolerance(
        self,
        target_time: str,
        execution_time: datetime,
        tolerance_minutes: int
    ) -> bool:
        """時刻が許容範囲内かチェックする。

        Args:
            target_time: 設定時刻("HH:MM"形式)
            execution_time: 実行時刻
            tolerance_minutes: 許容範囲(分)

        Returns:
            許容範囲内ならTrue
        """
        try:
            hour, minute = int(target_time[:2]), int(target_time[3:5])

            # target_timeを execution_time と同じ日付のdatetimeに変換
            target_dt = datetime(
                execution_time.year,
                execution_time.month,
                execution_time.day,
                hour,
                minute,
                tzinfo=execution_time.tzinfo
            )

            # 時間差を計算(秒単位)
            diff_seconds = abs((execution_time - target_dt).total_seconds())
            tolerance_seconds = tolerance_minutes * 60

            return diff_seconds <= tolerance_seconds
        except (ValueError, IndexError):
            return False

    @staticmethod
    def get_day_of_week(dt: datetime) -> str:
        """datetimeから曜日名を取得する。

        Args:
            dt: 日時

        Returns:
            曜日名("Monday", "Tuesday", ...)
        """
        days = [
            "Monday", "Tuesday", "Wednesday", "Thursday",
            "Friday", "Saturday", "Sunday"
        ]
        return days[dt.weekday()]
