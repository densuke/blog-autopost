"""スロット検索機能のテスト"""

from datetime import datetime
from unittest.mock import MagicMock

from src.timing_manager import TimingManager
from src.web.slot_finder import SlotFinder


class TestSlotFinderGenerateCandidateSlots:
    """候補スロット生成機能のテスト"""

    def test_generate_candidate_slots_today(self):
        """当日のスロット生成"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00", "12:00"],
            "Tuesday": ["09:00"],
            "Wednesday": ["09:00"],
            "Thursday": ["09:00"],
            "Friday": ["09:00"],
            "Saturday": ["09:00"],
            "Sunday": ["09:00"],
        }

        store = MagicMock()
        slot_finder = SlotFinder(timing_manager, store)

        # 月曜日09:00を開始時刻
        start_date = datetime(2025, 11, 10, 8, 0)  # 2025年11月10日は月曜日

        # --- Act ---
        result = slot_finder.generate_candidate_slots("x", start_date, 1)

        # --- Assert ---
        assert len(result) > 0
        # 09:00と12:00が候補に含まれる
        times = [dt.strftime("%H:%M") for dt in result]
        assert "09:00" in times
        assert "12:00" in times

    def test_generate_candidate_slots_multiple_days(self):
        """複数日のスロット生成"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00"],
            "Tuesday": ["10:00"],
            "Wednesday": ["11:00"],
        }

        store = MagicMock()
        slot_finder = SlotFinder(timing_manager, store)

        start_date = datetime(2025, 11, 10, 8, 0)  # 月曜日

        # --- Act ---
        result = slot_finder.generate_candidate_slots("x", start_date, 3)

        # --- Assert ---
        assert len(result) >= 3  # 3日分のスロット
        # 時系列順にソートされている
        for i in range(len(result) - 1):
            assert result[i] <= result[i + 1]

    def test_generate_candidate_slots_no_timings(self):
        """設定なしの場合空リスト"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = None

        store = MagicMock()
        slot_finder = SlotFinder(timing_manager, store)

        start_date = datetime(2025, 11, 10, 8, 0)

        # --- Act ---
        result = slot_finder.generate_candidate_slots("x", start_date, 1)

        # --- Assert ---
        assert result == []

    def test_generate_candidate_slots_skip_past(self):
        """過去時刻は除外される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["08:00", "09:00", "10:00"],
        }

        store = MagicMock()
        slot_finder = SlotFinder(timing_manager, store)

        # 月曜日09:00開始時刻
        start_date = datetime(2025, 11, 10, 9, 0)

        # --- Act ---
        result = slot_finder.generate_candidate_slots("x", start_date, 1)

        # --- Assert ---
        # 08:00は除外、09:00と10:00が含まれる
        times = [dt.strftime("%H:%M") for dt in result]
        assert "08:00" not in times
        assert "09:00" in times or "10:00" in times

    def test_generate_candidate_slots_timezone_normalized(self):
        """生成されるスロットはローカルタイムゾーンに正規化される。"""
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00"],
        }

        store = MagicMock()
        slot_finder = SlotFinder(timing_manager, store)

        start_date = datetime(2025, 11, 10, 8, 0)  # naive datetime
        result = slot_finder.generate_candidate_slots("x", start_date, 1)

        assert len(result) == 1
        assert result[0].tzinfo is not None


class TestSlotFinderIsSlotAvailable:
    """スロット空き状況確認のテスト"""

    def test_is_slot_available_empty(self):
        """空きスロット判定"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        store = MagicMock()
        store.get_posts_by_sns_and_time.return_value = []

        slot_finder = SlotFinder(timing_manager, store)

        slot_time = datetime(2025, 11, 10, 9, 0)

        # --- Act ---
        result = slot_finder.is_slot_available("x", slot_time)

        # --- Assert ---
        assert result is True

    def test_is_slot_available_occupied(self):
        """埋まっているスロット判定"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        store = MagicMock()

        # 既存の予約を返す
        existing_post = MagicMock()
        store.get_posts_by_sns_and_time.return_value = [existing_post]

        slot_finder = SlotFinder(timing_manager, store)

        slot_time = datetime(2025, 11, 10, 9, 0)

        # --- Act ---
        result = slot_finder.is_slot_available("x", slot_time)

        # --- Assert ---
        assert result is False

    def test_is_slot_available_passes_tolerance(self):
        """許容範囲がデータストア呼び出しに反映される。"""
        timing_manager = TimingManager(None)
        store = MagicMock()
        store.get_posts_by_sns_and_time.return_value = []

        slot_finder = SlotFinder(timing_manager, store, tolerance_minutes=10)
        slot_time = datetime(2025, 11, 10, 9, 0)

        slot_finder.is_slot_available("x", slot_time)

        store.get_posts_by_sns_and_time.assert_called_once()
        args, kwargs = store.get_posts_by_sns_and_time.call_args
        assert args[0] == "x"
        normalized_time = args[1]
        assert normalized_time.tzinfo is not None
        assert normalized_time.hour == slot_time.hour
        assert kwargs["tolerance_minutes"] == 10


class TestSlotFinderFindNextAvailableSlot:
    """次の空きスロット検索のテスト"""

    def test_find_next_available_slot_today(self):
        """当日の空きスロット検索"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00", "12:00"],
        }

        store = MagicMock()
        store.get_posts_by_sns_and_time.return_value = []

        slot_finder = SlotFinder(timing_manager, store)

        start_from = datetime(2025, 11, 10, 8, 0)  # 月曜日8時

        # --- Act ---
        result = slot_finder.find_next_available_slot("x", start_from, 7)

        # --- Assert ---
        assert result is not None
        assert result.strftime("%H:%M") in ["09:00", "12:00"]

    def test_find_next_available_slot_conflict(self):
        """競合スロットをスキップ"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00", "12:00"],
        }

        store = MagicMock()

        def mock_get_posts(sns, time, **kwargs):
            # 09:00は埋まっている
            if time.strftime("%H:%M") == "09:00":
                return [MagicMock()]
            return []

        store.get_posts_by_sns_and_time.side_effect = mock_get_posts

        slot_finder = SlotFinder(timing_manager, store)

        start_from = datetime(2025, 11, 10, 8, 0)

        # --- Act ---
        result = slot_finder.find_next_available_slot("x", start_from, 7)

        # --- Assert ---
        assert result is not None
        assert result.strftime("%H:%M") == "12:00"  # 次の空きスロット

    def test_find_next_available_slot_no_slot(self):
        """7日以内に空きなし"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00"],
        }

        store = MagicMock()
        store.get_posts_by_sns_and_time.return_value = [MagicMock()]

        slot_finder = SlotFinder(timing_manager, store)

        start_from = datetime(2025, 11, 10, 8, 0)

        # --- Act ---
        result = slot_finder.find_next_available_slot("x", start_from, 7)

        # --- Assert ---
        assert result is None

    def test_find_next_available_slot_no_config(self):
        """設定なしの場合は制限なしモードで現在時刻より後の時刻を返す"""
        # --- Arrange ---
        from src.web.timezone_utils import ensure_local_timezone
        
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = None

        store = MagicMock()
        slot_finder = SlotFinder(timing_manager, store)

        start_from = datetime(2025, 11, 10, 8, 0)
        start_from_tz = ensure_local_timezone(start_from)

        # --- Act ---
        result = slot_finder.find_next_available_slot("x", start_from, 7)

        # --- Assert ---
        # 設定なし時は現在時刻より後の時刻を返す
        assert result is not None
        assert result > start_from_tz
        # 1分以上後の時刻を返す
        from datetime import timedelta
        assert result >= start_from_tz + timedelta(minutes=1)


class TestSlotFinderFindSlotsForMultipleSns:
    """複数SNS一括検索のテスト"""

    def test_find_slots_for_multiple_sns_all_success(self):
        """全SNSで空きあり"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {"Monday": ["09:00"]}
        timing_manager._timing_cache["bluesky"] = {"Monday": ["10:00"]}

        store = MagicMock()
        store.get_posts_by_sns_and_time.return_value = []

        slot_finder = SlotFinder(timing_manager, store)

        # --- Act ---
        result = slot_finder.find_slots_for_multiple_sns(["x", "bluesky"])

        # --- Assert ---
        assert "x" in result
        assert "bluesky" in result
        assert result["x"] is not None
        assert result["bluesky"] is not None

    def test_find_slots_for_multiple_sns_partial_failure(self):
        """一部SNSで投稿可能タイミング設定あり、一部なし"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {"Monday": ["09:00"]}
        timing_manager._timing_cache["bluesky"] = None  # 設定なし = 制限なしモード

        store = MagicMock()
        store.get_posts_by_sns_and_time.return_value = []

        slot_finder = SlotFinder(timing_manager, store)

        # --- Act ---
        result = slot_finder.find_slots_for_multiple_sns(["x", "bluesky"])

        # --- Assert ---
        assert result["x"] is not None
        # 設定なし時は制限なしモードで現在時刻より後の時刻を返す
        assert result["bluesky"] is not None  # 設定なし


class TestSlotFinderToleranceHandling:
    """tolerance_minutesを考慮したスロット検索のテスト"""

    def test_generate_candidate_slots_with_tolerance(self):
        """tolerance_minutesを考慮して近すぎるスロットをスキップする"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["12:10", "12:15", "12:20", "12:25"],
        }

        store = MagicMock()
        # tolerance_minutes=5分で初期化
        slot_finder = SlotFinder(timing_manager, store, tolerance_minutes=5)

        # 月曜日12:11を開始時刻(12:15のスロットまで4分)
        start_date = datetime(2025, 11, 10, 12, 11)  # 2025年11月10日は月曜日

        # --- Act ---
        result = slot_finder.generate_candidate_slots("x", start_date, 1)

        # --- Assert ---
        # 12:11 + 5分 = 12:16より後のスロットのみが候補
        times = [dt.strftime("%H:%M") for dt in result]
        assert "12:10" not in times  # 過去
        assert "12:15" not in times  # 近すぎる(12:16より前)
        assert "12:20" in times       # OK
        assert "12:25" in times       # OK

    def test_find_next_available_slot_skips_near_past_slots(self):
        """過去に近いスロットを正しくスキップして次のスロットを返す"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["12:15", "12:25", "12:35"],
        }

        store = MagicMock()
        store.get_posts_by_sns_and_time.return_value = []  # 全スロット空き

        # tolerance_minutes=5分で初期化
        slot_finder = SlotFinder(timing_manager, store, tolerance_minutes=5)

        # 月曜日12:15を過ぎた時刻(12:20)から検索
        start_date = datetime(2025, 11, 10, 12, 20)  # 2025年11月10日は月曜日

        # --- Act ---
        result = slot_finder.find_next_available_slot("x", start_date)

        # --- Assert ---
        # 12:20 + 5分 = 12:25より後のスロット → 12:35
        assert result is not None
        assert result.strftime("%H:%M") == "12:35"

    def test_find_next_available_slot_with_zero_tolerance(self):
        """tolerance_minutes=0の場合は現在時刻より後のスロットを返す"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["12:15", "12:20"],
        }

        store = MagicMock()
        store.get_posts_by_sns_and_time.return_value = []

        # tolerance_minutes=0で初期化
        slot_finder = SlotFinder(timing_manager, store, tolerance_minutes=0)

        # 月曜日12:15の直後(12:15:01)から検索
        start_date = datetime(2025, 11, 10, 12, 15, 1)

        # --- Act ---
        result = slot_finder.find_next_available_slot("x", start_date)

        # --- Assert ---
        # tolerance=0なので、12:15:01より後の最初のスロット → 12:20
        assert result is not None
        assert result.strftime("%H:%M") == "12:20"
