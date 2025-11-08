"""タイミング検証機能のテスト"""

from datetime import datetime

from src.timing_manager import TimingManager
from src.web.timing_validator import TimingValidator


class TestTimingValidatorGetDayOfWeek:
    """曜日取得機能のテスト"""

    def test_get_day_of_week_monday(self):
        """月曜日の取得"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        validator = TimingValidator(timing_manager)

        # 2025年11月10日は月曜日
        dt = datetime(2025, 11, 10, 9, 0)

        # --- Act ---
        result = validator.get_day_of_week(dt)

        # --- Assert ---
        assert result == "Monday"

    def test_get_day_of_week_sunday(self):
        """日曜日の取得"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        validator = TimingValidator(timing_manager)

        # 2025年11月9日は日曜日
        dt = datetime(2025, 11, 9, 9, 0)

        # --- Act ---
        result = validator.get_day_of_week(dt)

        # --- Assert ---
        assert result == "Sunday"

    def test_get_day_of_week_all_days(self):
        """全曜日をテスト"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        validator = TimingValidator(timing_manager)

        # 2025年11月10日(月)から始まる週
        days = [
            (datetime(2025, 11, 10), "Monday"),
            (datetime(2025, 11, 11), "Tuesday"),
            (datetime(2025, 11, 12), "Wednesday"),
            (datetime(2025, 11, 13), "Thursday"),
            (datetime(2025, 11, 14), "Friday"),
            (datetime(2025, 11, 15), "Saturday"),
            (datetime(2025, 11, 16), "Sunday"),
        ]

        # --- Act & Assert ---
        for dt, expected_day in days:
            result = validator.get_day_of_week(dt)
            assert result == expected_day


class TestTimingValidatorIsTimeWithinTolerance:
    """時刻許容範囲チェックのテスト"""

    def test_is_time_within_tolerance_exact(self):
        """完全一致"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        validator = TimingValidator(timing_manager, tolerance_minutes=5)

        execution_time = datetime(2025, 11, 10, 9, 0)

        # --- Act ---
        result = validator.is_time_within_tolerance("09:00", execution_time, 5)

        # --- Assert ---
        assert result is True

    def test_is_time_within_tolerance_within(self):
        """許容範囲内(±5分)"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        validator = TimingValidator(timing_manager, tolerance_minutes=5)

        # 実行時刻: 09:02
        execution_time = datetime(2025, 11, 10, 9, 2)

        # --- Act ---
        result = validator.is_time_within_tolerance("09:00", execution_time, 5)

        # --- Assert ---
        assert result is True

    def test_is_time_within_tolerance_out(self):
        """許容範囲外"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        validator = TimingValidator(timing_manager, tolerance_minutes=5)

        # 実行時刻: 09:10
        execution_time = datetime(2025, 11, 10, 9, 10)

        # --- Act ---
        result = validator.is_time_within_tolerance("09:00", execution_time, 5)

        # --- Assert ---
        assert result is False

    def test_is_time_within_tolerance_zero(self):
        """許容範囲0分(厳密一致)"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        validator = TimingValidator(timing_manager, tolerance_minutes=0)

        # 実行時刻: 09:00
        execution_time = datetime(2025, 11, 10, 9, 0)

        # --- Act ---
        result = validator.is_time_within_tolerance("09:00", execution_time, 0)

        # --- Assert ---
        assert result is True

    def test_is_time_within_tolerance_zero_not_exact(self):
        """許容範囲0分で1分ずれ"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        validator = TimingValidator(timing_manager, tolerance_minutes=0)

        # 実行時刻: 09:01
        execution_time = datetime(2025, 11, 10, 9, 1)

        # --- Act ---
        result = validator.is_time_within_tolerance("09:00", execution_time, 0)

        # --- Assert ---
        assert result is False


class TestTimingValidatorValidateTiming:
    """タイミング検証機能のテスト"""

    def test_validate_timing_within_range(self):
        """許可範囲内の検証"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00", "12:00"],
        }

        validator = TimingValidator(timing_manager, tolerance_minutes=5)

        # 実行時刻: 月曜日09:02
        execution_time = datetime(2025, 11, 10, 9, 2)

        # --- Act ---
        valid, reason = validator.validate_timing("x", execution_time)

        # --- Assert ---
        assert valid is True
        assert reason is None

    def test_validate_timing_out_of_range(self):
        """許可範囲外の検証"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00", "12:00"],
        }

        validator = TimingValidator(timing_manager, tolerance_minutes=5)

        # 実行時刻: 月曜日15:00
        execution_time = datetime(2025, 11, 10, 15, 0)

        # --- Act ---
        valid, reason = validator.validate_timing("x", execution_time)

        # --- Assert ---
        assert valid is False
        assert reason is not None

    def test_validate_timing_tolerance(self):
        """許容範囲(±5分)での検証"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00"],
        }

        validator = TimingValidator(timing_manager, tolerance_minutes=5)

        # 実行時刻: 月曜日08:57 (09:00の3分前)
        execution_time = datetime(2025, 11, 10, 8, 57)

        # --- Act ---
        valid, reason = validator.validate_timing("x", execution_time)

        # --- Assert ---
        assert valid is True
        assert reason is None

    def test_validate_timing_no_config(self):
        """設定なしは常にTrue"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = None

        validator = TimingValidator(timing_manager, tolerance_minutes=5)

        execution_time = datetime(2025, 11, 10, 15, 0)

        # --- Act ---
        valid, reason = validator.validate_timing("x", execution_time)

        # --- Assert ---
        assert valid is True
        assert reason is None

    def test_validate_timing_wrong_day(self):
        """設定のない曜日"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00"],
        }

        validator = TimingValidator(timing_manager, tolerance_minutes=5)

        # 実行時刻: 火曜日09:00(設定がない曜日)
        execution_time = datetime(2025, 11, 11, 9, 0)

        # --- Act ---
        valid, reason = validator.validate_timing("x", execution_time)

        # --- Assert ---
        assert valid is False
        assert reason is not None

    def test_validate_timing_multiple_times(self):
        """複数時刻がある場合、いずれか一つに合致"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00", "12:00", "15:00"],
        }

        validator = TimingValidator(timing_manager, tolerance_minutes=5)

        # 実行時刻: 月曜日12:00
        execution_time = datetime(2025, 11, 10, 12, 0)

        # --- Act ---
        valid, reason = validator.validate_timing("x", execution_time)

        # --- Assert ---
        assert valid is True
        assert reason is None
