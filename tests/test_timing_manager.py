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
