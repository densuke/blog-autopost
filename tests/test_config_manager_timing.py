"""ConfigManagerのタイミング設定拡張機能のテスト"""

from src.config_manager import ConfigManager


class TestConfigManagerTimingSettings:
    """タイミング設定の読み込みテスト"""

    def test_load_default_allowed_timings(self):
        """グローバル設定(default_allowed_timings)を読み込める"""
        # --- Arrange ---
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'default_allowed_timings': [
                ['*', ['18:00', '20:00']],
                ['Weekday', ['09:00', '12:00']]
            ],
            'sns': {}
        }
        config_manager = ConfigManager(config)

        # --- Act ---
        result = config_manager.get_default_allowed_timings()

        # --- Assert ---
        assert result is not None
        assert len(result) == 2
        assert result[0] == ['*', ['18:00', '20:00']]
        assert result[1] == ['Weekday', ['09:00', '12:00']]

    def test_load_default_allowed_timings_not_set(self):
        """default_allowed_timingsが設定されていない場合Noneを返す"""
        # --- Arrange ---
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'sns': {}
        }
        config_manager = ConfigManager(config)

        # --- Act ---
        result = config_manager.get_default_allowed_timings()

        # --- Assert ---
        assert result is None

    def test_load_tolerance_minutes_default(self):
        """許容時間のデフォルト値は5分"""
        # --- Arrange ---
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'sns': {}
        }
        config_manager = ConfigManager(config)

        # --- Act ---
        result = config_manager.get_allowed_timings_tolerance_minutes()

        # --- Assert ---
        assert result == 5

    def test_load_tolerance_minutes_custom(self):
        """カスタムの許容時間を読み込める"""
        # --- Arrange ---
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'allowed_timings_tolerance_minutes': 10,
            'sns': {}
        }
        config_manager = ConfigManager(config)

        # --- Act ---
        result = config_manager.get_allowed_timings_tolerance_minutes()

        # --- Assert ---
        assert result == 10

    def test_load_tolerance_minutes_zero(self):
        """許容時間0分(厳密一致)を読み込める"""
        # --- Arrange ---
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'allowed_timings_tolerance_minutes': 0,
            'sns': {}
        }
        config_manager = ConfigManager(config)

        # --- Act ---
        result = config_manager.get_allowed_timings_tolerance_minutes()

        # --- Assert ---
        assert result == 0

    def test_load_sns_allowed_timings_array_format(self):
        """配列形式のSNS設定からallowed_timingsを読み込める"""
        # --- Arrange ---
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'sns': [
                {
                    'type': 'x',
                    'name': 'x',
                    'allowed_timings': [
                        ['Monday', ['09:00', '12:00']],
                        ['Wednesday', ['15:00']]
                    ]
                },
                {
                    'type': 'bluesky',
                    'name': 'bluesky'
                    # allowed_timingsなし
                }
            ]
        }
        config_manager = ConfigManager(config)

        # --- Act ---
        sns_configs = config_manager.get_all_sns_configs()

        # --- Assert ---
        assert isinstance(sns_configs, list)
        x_config = next((c for c in sns_configs if c.get('name') == 'x'), None)
        assert x_config is not None
        assert 'allowed_timings' in x_config
        assert len(x_config['allowed_timings']) == 2
        assert x_config['allowed_timings'][0] == ['Monday', ['09:00', '12:00']]

        # Blueskyはallowed_timingsなし
        bs_config = next((c for c in sns_configs if c.get('name') == 'bluesky'), None)
        assert bs_config is not None
        assert 'allowed_timings' not in bs_config or bs_config.get('allowed_timings') is None

    def test_load_sns_allowed_timings_dict_format(self):
        """オブジェクト形式のSNS設定からallowed_timingsを読み込める"""
        # --- Arrange ---
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'sns': {
                'x': {
                    'api_key': 'xxx',
                    'allowed_timings': [
                        ['*', ['09:00']]
                    ]
                },
                'bluesky': {
                    'handle': 'user.bsky.social'
                }
            }
        }
        config_manager = ConfigManager(config)

        # --- Act ---
        sns_configs = config_manager.get_all_sns_configs()

        # --- Assert ---
        assert isinstance(sns_configs, dict)
        assert 'allowed_timings' in sns_configs['x']
        assert sns_configs['x']['allowed_timings'][0] == ['*', ['09:00']]

    def test_backward_compatibility_no_timing_fields(self):
        """新フィールド未設定でも既存の設定は読み込める"""
        # --- Arrange ---
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'sns': {
                'x': {
                    'api_key': 'xxx'
                }
            }
        }
        config_manager = ConfigManager(config)

        # --- Act ---
        feed_url = config_manager.get_feed_url()
        sns_configs = config_manager.get_all_sns_configs()
        default_timings = config_manager.get_default_allowed_timings()
        tolerance = config_manager.get_allowed_timings_tolerance_minutes()

        # --- Assert ---
        assert feed_url == 'http://test.com/feed'
        assert 'x' in sns_configs
        assert default_timings is None
        assert tolerance == 5  # デフォルト値

    def test_all_timing_settings_together(self):
        """グローバル設定、カスタム許容時間、SNS固有設定がすべて揃っている場合"""
        # --- Arrange ---
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'default_allowed_timings': [
                ['*', ['18:00']]
            ],
            'allowed_timings_tolerance_minutes': 3,
            'sns': [
                {
                    'type': 'x',
                    'name': 'x',
                    'allowed_timings': [
                        ['Monday', ['09:00']]
                    ]
                }
            ]
        }
        config_manager = ConfigManager(config)

        # --- Act ---
        default_timings = config_manager.get_default_allowed_timings()
        tolerance = config_manager.get_allowed_timings_tolerance_minutes()
        sns_configs = config_manager.get_all_sns_configs()

        # --- Assert ---
        assert default_timings == [['*', ['18:00']]]
        assert tolerance == 3
        assert sns_configs[0]['allowed_timings'] == [['Monday', ['09:00']]]

    def test_find_sns_config_list_format(self):
        """配列形式のSNS設定から特定エントリを取得できる"""
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'sns': [
                {'type': 'x', 'name': 'x', 'allowed_timings': [['Monday', ['09:00']]]}
            ]
        }
        config_manager = ConfigManager(config)

        entry = config_manager.find_sns_config('x')
        assert entry is not None
        assert entry['type'] == 'x'

    def test_find_sns_config_dict_format(self):
        """辞書形式のSNS設定から特定エントリを取得できる"""
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'sns': {
                'bluesky': {'type': 'bluesky'}
            }
        }
        config_manager = ConfigManager(config)

        entry = config_manager.find_sns_config('bluesky')
        assert entry is not None
        assert entry['type'] == 'bluesky'

    def test_get_allowed_timings_map_empty(self):
        """allowed_timingsセクション未設定時は空辞書を返す"""
        config = {
            'blog': {'feed_url': 'http://test.com/feed'}
        }
        config_manager = ConfigManager(config)

        assert config_manager.get_allowed_timings_map() == {}

    def test_get_allowed_timings_map_with_values(self):
        """allowed_timingsセクションをそのまま取得できる"""
        config = {
            'blog': {'feed_url': 'http://test.com/feed'},
            'allowed_timings': {
                'bluesky': [['*', ['05:00']]]
            }
        }
        config_manager = ConfigManager(config)

        allowed = config_manager.get_allowed_timings_map()
        assert isinstance(allowed, dict)
        assert 'bluesky' in allowed
        assert allowed['bluesky'][0] == ['*', ['05:00']]
