from unittest.mock import MagicMock
from src.timing_manager import TimingManager


class TestTimingManagerExpandWildcard:
    """ワイルドカード展開機能のテスト"""

    def test_expand_wildcard_asterisk(self):
        """'*'がすべての曜日に展開される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)

        # --- Act ---
        result = timing_manager.expand_wildcard("*")

        # --- Assert ---
        expected = [
            "Monday", "Tuesday", "Wednesday", "Thursday",
            "Friday", "Saturday", "Sunday"
        ]
        assert result == expected

    def test_expand_wildcard_weekday(self):
        """'Weekday'が月〜金に展開される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)

        # --- Act ---
        result = timing_manager.expand_wildcard("Weekday")

        # --- Assert ---
        expected = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"]
        assert result == expected

    def test_expand_wildcard_weekend(self):
        """'Weekend'が土日に展開される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)

        # --- Act ---
        result = timing_manager.expand_wildcard("Weekend")

        # --- Assert ---
        expected = ["Saturday", "Sunday"]
        assert result == expected

    def test_expand_wildcard_specific_day(self):
        """特定曜日はそのまま返される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)

        # --- Act ---
        result = timing_manager.expand_wildcard("Monday")

        # --- Assert ---
        assert result == ["Monday"]

    def test_expand_wildcard_all_days(self):
        """各曜日をテスト"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]

        # --- Act & Assert ---
        for day in days:
            result = timing_manager.expand_wildcard(day)
            assert result == [day]


class TestTimingManagerMergeTimings:
    """タイミング統合(和集合)機能のテスト"""

    def test_merge_timings_union(self):
        """グローバル設定とSNS固有設定が和集合で統合される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        global_timings = [["*", ["18:00", "20:00"]]]
        sns_timings = [["Monday", ["09:00", "12:00"]]]

        # --- Act ---
        result = timing_manager.merge_timings(global_timings, sns_timings)

        # --- Assert ---
        assert "Monday" in result
        assert set(result["Monday"]) == {"09:00", "12:00", "18:00", "20:00"}
        assert "Tuesday" in result
        assert result["Tuesday"] == ["18:00", "20:00"]

    def test_merge_timings_duplicate(self):
        """重複時刻が1つにまとめられる"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        global_timings = [["Monday", ["09:00", "12:00"]]]
        sns_timings = [["Monday", ["09:00", "15:00"]]]

        # --- Act ---
        result = timing_manager.merge_timings(global_timings, sns_timings)

        # --- Assert ---
        assert result["Monday"] == ["09:00", "12:00", "15:00"]

    def test_merge_timings_empty_global(self):
        """グローバル設定がない場合"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        global_timings = []
        sns_timings = [["Monday", ["09:00", "12:00"]]]

        # --- Act ---
        result = timing_manager.merge_timings(global_timings, sns_timings)

        # --- Assert ---
        assert result == {"Monday": ["09:00", "12:00"]}

    def test_merge_timings_empty_sns(self):
        """SNS固有設定がない場合"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        global_timings = [["*", ["18:00"]]]
        sns_timings = []

        # --- Act ---
        result = timing_manager.merge_timings(global_timings, sns_timings)

        # --- Assert ---
        assert result["Monday"] == ["18:00"]
        assert result["Sunday"] == ["18:00"]

    def test_merge_timings_wildcard(self):
        """ワイルドカードを含む統合"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        global_timings = [["Weekday", ["09:00"]], ["Weekend", ["10:00"]]]
        sns_timings = [["Monday", ["12:00"]]]

        # --- Act ---
        result = timing_manager.merge_timings(global_timings, sns_timings)

        # --- Assert ---
        # Monday: Weekday + SNS固有
        assert set(result["Monday"]) == {"09:00", "12:00"}
        # Tuesday: Weekdayのみ
        assert result["Tuesday"] == ["09:00"]
        # Saturday: Weekendのみ
        assert result["Saturday"] == ["10:00"]

    def test_merge_timings_sorted(self):
        """時刻がソート済みで返される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        global_timings = [["Monday", ["20:00", "09:00"]]]
        sns_timings = [["Monday", ["15:00"]]]

        # --- Act ---
        result = timing_manager.merge_timings(global_timings, sns_timings)

        # --- Assert ---
        assert result["Monday"] == ["09:00", "15:00", "20:00"]


class TestTimingManagerValidateTimingConfig:
    """設定バリデーション機能のテスト"""

    def test_validate_timing_config_valid(self):
        """正常な設定は妥当と判定される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timings = [
            ["Monday", ["09:00", "12:00"]],
            ["*", ["18:00"]],
            ["Weekday", ["15:00"]]
        ]

        # --- Act ---
        valid, errors = timing_manager.validate_timing_config(timings)

        # --- Assert ---
        assert valid is True
        assert errors == []

    def test_validate_timing_config_invalid_time(self):
        """無効時刻は検出される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timings = [
            ["Monday", ["25:00"]],  # 無効
            ["Tuesday", ["12:60"]],  # 無効
        ]

        # --- Act ---
        valid, errors = timing_manager.validate_timing_config(timings)

        # --- Assert ---
        assert valid is False
        assert len(errors) == 2
        assert any("25:00" in error for error in errors)
        assert any("12:60" in error for error in errors)

    def test_validate_timing_config_invalid_day(self):
        """無効曜日は検出される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timings = [
            ["Moonday", ["09:00"]],  # 無効
            ["InvalidDay", ["12:00"]],  # 無効
        ]

        # --- Act ---
        valid, errors = timing_manager.validate_timing_config(timings)

        # --- Assert ---
        assert valid is False
        assert len(errors) == 2
        assert any("Moonday" in error for error in errors)

    def test_validate_timing_config_invalid_format(self):
        """フォーマット違反は検出される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timings = [
            ["Monday", ["9:00"]],  # 1桁時間
            ["Tuesday", ["09-00"]],  # ハイフン区切り
            ["Wednesday", ["0900"]],  # コロンなし
        ]

        # --- Act ---
        valid, errors = timing_manager.validate_timing_config(timings)

        # --- Assert ---
        assert valid is False
        assert len(errors) == 3

    def test_validate_timing_config_boundary(self):
        """境界値は正常に処理される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timings = [
            ["Monday", ["00:00", "23:59"]],  # 最小・最大値
        ]

        # --- Act ---
        valid, errors = timing_manager.validate_timing_config(timings)

        # --- Assert ---
        assert valid is True
        assert errors == []

    def test_validate_timing_config_mixed(self):
        """有効・無効な設定が混在する場合"""
        # --- Arrange ---
        timing_manager = TimingManager(None)
        timings = [
            ["Monday", ["09:00"]],  # 有効
            ["InvalidDay", ["12:00"]],  # 無効
            ["Tuesday", ["25:00"]],  # 無効
        ]

        # --- Act ---
        valid, errors = timing_manager.validate_timing_config(timings)

        # --- Assert ---
        assert valid is False
        assert len(errors) == 2


class TestTimingManagerGetAllowedTimings:
    """SNS別タイミング情報取得のテスト"""

    def test_get_allowed_timings_no_config(self):
        """設定がない場合Noneを返す"""
        # --- Arrange ---
        timing_manager = TimingManager(None)

        # --- Act ---
        result = timing_manager.get_allowed_timings("unknown_sns")

        # --- Assert ---
        assert result is None

    def test_get_allowed_timings_cache(self):
        """2回目以降はキャッシュから取得される"""
        # --- Arrange ---
        timing_manager = TimingManager(None)

        # --- Act ---
        result1 = timing_manager.get_allowed_timings("test_sns")
        result2 = timing_manager.get_allowed_timings("test_sns")

        # --- Assert ---
        assert result1 is result2  # 同じオブジェクト

    def test_get_allowed_timings_with_both(self):
        """グローバルとSNS固有設定がある場合"""
        # --- Arrange ---
        mock_config_manager = MagicMock()
        timing_manager = TimingManager(mock_config_manager)

        # ConfigManagerのモック設定(後続タスクで実装予定のメソッド)
        timing_manager._timing_cache["x"] = {
            "Monday": ["09:00", "12:00", "18:00"],
            "Tuesday": ["18:00"]
        }

        # --- Act ---
        result = timing_manager.get_allowed_timings("x")

        # --- Assert ---
        assert result == {
            "Monday": ["09:00", "12:00", "18:00"],
            "Tuesday": ["18:00"]
        }

    def test_get_allowed_timings_multiple_sns(self):
        """複数SNSのキャッシュが独立している"""
        # --- Arrange ---
        timing_manager = TimingManager(None)

        # キャッシュに直接設定(実装時はConfigManagerから読み込み)
        timing_manager._timing_cache["x"] = {"Monday": ["09:00"]}
        timing_manager._timing_cache["bluesky"] = {"Monday": ["10:00"]}

        # --- Act ---
        result_x = timing_manager.get_allowed_timings("x")
        result_bs = timing_manager.get_allowed_timings("bluesky")

        # --- Assert ---
        assert result_x == {"Monday": ["09:00"]}
        assert result_bs == {"Monday": ["10:00"]}
