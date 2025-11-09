"""投稿可能タイミング設定の管理モジュール。

Bufferスタイルの投稿タイミング管理システムを実装します。
各SNSアカウントに曜日別・時刻別の投稿可能タイミングを設定し、
グローバル設定とSNS固有設定を統合して利用可能スロットを提供します。
"""

from typing import Optional, Dict, List, Tuple
import logging

logger = logging.getLogger(__name__)


class TimingManager:
    """投稿可能タイミング設定の管理と提供。

    config.ymlから投稿タイミング設定を読み込み、ワイルドカード展開、
    バリデーション、グローバル設定とSNS固有設定の統合を行います。
    """

    def __init__(self, config_manager):
        """TimingManagerの初期化。

        Args:
            config_manager: ConfigManagerインスタンス(後続タスクで使用)
        """
        self.config_manager = config_manager
        self._timing_cache: Dict[str, Optional[Dict[str, List[str]]]] = {}

    def expand_wildcard(self, day_spec: str) -> List[str]:
        """ワイルドカードを具体的な曜日リストに展開する。

        Args:
            day_spec: 曜日指定文字列
                - "*": 全曜日(月〜日)
                - "Weekday": 平日(月〜金)
                - "Weekend": 週末(土日)
                - その他: 特定曜日("Monday", "Tuesday", ...)

        Returns:
            曜日リスト。例: ["Monday", "Tuesday", "Wednesday", ...]
        """
        if day_spec == "*":
            return [
                "Monday", "Tuesday", "Wednesday", "Thursday",
                "Friday", "Saturday", "Sunday"
            ]

        if day_spec == "Weekday":
            return ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"]

        if day_spec == "Weekend":
            return ["Saturday", "Sunday"]

        # 特定曜日はそのまま返す
        return [day_spec]

    def merge_timings(
        self,
        global_timings: List[Tuple[str, List[str]]],
        sns_timings: List[Tuple[str, List[str]]]
    ) -> Dict[str, List[str]]:
        """グローバル設定とSNS固有設定をマージする(和集合)。

        Args:
            global_timings: default_allowed_timings(グローバル設定)
            sns_timings: SNS固有のallowed_timings

        Returns:
            統合済みの投稿可能タイミング。
            例: {"Monday": ["09:00", "12:00"], "Tuesday": ["18:00"]}
        """
        result: Dict[str, set] = {}

        # グローバル設定を展開・追加
        for day_spec, times in global_timings:
            expanded_days = self.expand_wildcard(day_spec)
            for day in expanded_days:
                if day not in result:
                    result[day] = set()
                result[day].update(times)

        # SNS固有設定を展開・追加(和集合)
        for day_spec, times in sns_timings:
            expanded_days = self.expand_wildcard(day_spec)
            for day in expanded_days:
                if day not in result:
                    result[day] = set()
                result[day].update(times)

        # セットをソート済みリストに変換
        return {
            day: sorted(list(times))
            for day, times in result.items()
        }

    def validate_timing_config(
        self,
        timings: List[Tuple[str, List[str]]]
    ) -> Tuple[bool, List[str]]:
        """タイミング設定のバリデーション。

        Args:
            timings: 検証対象のタイミング設定

        Returns:
            (valid, error_messages)のタプル
            - valid: 有効ならTrue、エラーあればFalse
            - error_messages: エラーメッセージリスト
        """
        errors: List[str] = []
        valid_days = {
            "Monday", "Tuesday", "Wednesday", "Thursday",
            "Friday", "Saturday", "Sunday",
            "*", "Weekday", "Weekend"
        }

        for day_spec, times in timings:
            # 曜日指定のバリデーション
            if day_spec not in valid_days:
                errors.append(f"無効な曜日指定: '{day_spec}'")
                continue

            # 時刻フォーマットのバリデーション
            for time_str in times:
                if not self._is_valid_time_format(time_str):
                    errors.append(f"無効な時刻フォーマット: '{time_str}'")

        return len(errors) == 0, errors

    @staticmethod
    def _is_valid_time_format(time_str: str) -> bool:
        """時刻フォーマットが有効か確認する。

        Args:
            time_str: チェック対象の時刻文字列("HH:MM"形式)

        Returns:
            有効ならTrue
        """
        try:
            if not isinstance(time_str, str):
                return False

            if len(time_str) != 5 or time_str[2] != ":":
                return False

            hour = int(time_str[0:2])
            minute = int(time_str[3:5])

            # 時間の範囲チェック(0:00 〜 23:59)
            if not (0 <= hour <= 23):
                return False

            # 分の範囲チェック(0 〜 59)
            if not (0 <= minute <= 59):
                return False

            return True
        except (ValueError, IndexError):
            return False

    def get_allowed_timings(self, sns_name: str) -> Optional[Dict[str, List[str]]]:
        """指定されたSNSの投稿可能タイミングを取得する。

        ConfigManagerからグローバル設定とSNS固有設定を読み込み、
        統合してキャッシュに保存します。

        Args:
            sns_name: SNS名

        Returns:
            曜日をキー、時刻リストを値とする辞書。
            設定がない場合(両方未定義)はNone。
            例: {"Monday": ["09:00", "12:00"], "Tuesday": ["18:00"]}
        """
        # キャッシュから取得
        if sns_name in self._timing_cache:
            return self._timing_cache[sns_name]

        if not self.config_manager:
            self._timing_cache[sns_name] = None
            return None

        global_timings = self.config_manager.get_default_allowed_timings() or []
        allowed_map = self.config_manager.get_allowed_timings_map()
        sns_specific_timings = allowed_map.get(sns_name) if isinstance(allowed_map, dict) else None

        extra_timings: List[Tuple[str, List[str]]] = []
        if isinstance(sns_specific_timings, list):
            extra_timings.extend(sns_specific_timings)

        sns_entry = self.config_manager.find_sns_config(sns_name)
        if sns_entry:
            entry_timings = sns_entry.get('allowed_timings')
            if isinstance(entry_timings, list):
                extra_timings.extend(entry_timings)

        if not global_timings and not extra_timings:
            self._timing_cache[sns_name] = None
            return None

        merged_timings = self.merge_timings(global_timings, extra_timings)
        self._timing_cache[sns_name] = merged_timings
        return merged_timings
