import pytest
import yaml
from unittest.mock import patch, mock_open
from src.config_manager import ConfigManager, load_config
from src.plugin_loader import load_plugins


class TestMultipleAccountsConfig:
    """複数アカウント対応の設定読み込みテスト"""

    def test_load_config_array_format(self):
        """配列形式の設定読み込みテスト"""
        config_content = """
sns:
  - type: mastodon
    name: "mastodon-main"
    instance_url: "https://mastodon.social"
    access_token: "token1"
  - type: mastodon
    name: "mstdn-jp"
    instance_url: "https://mstdn.jp"
    access_token: "token2"
  - type: x
    name: "x-personal"
    consumer_key: "key1"
    consumer_secret: "secret1"
    access_token: "token1"
    access_token_secret: "tokensecret1"
"""
        with patch("builtins.open", mock_open(read_data=config_content)):
            config = load_config("test_config.yml")
            config_manager = ConfigManager(config)
            
            # 配列形式でSNS設定が読み込まれることを確認
            assert isinstance(config['sns'], list)
            assert len(config['sns']) == 3
            
            # 各アカウントの設定確認
            mastodon_accounts = [sns for sns in config['sns'] if sns['type'] == 'mastodon']
            assert len(mastodon_accounts) == 2
            assert mastodon_accounts[0]['name'] == "mastodon-main"
            assert mastodon_accounts[1]['name'] == "mstdn-jp"

    def test_load_config_object_format_backward_compatibility(self):
        """オブジェクト形式の設定（後方互換性）テスト"""
        config_content = """
sns:
  mastodon:
    instance_url: "https://mastodon.social"
    access_token: "token"
  x:
    consumer_key: "key"
    consumer_secret: "secret"
    access_token: "token"
    access_token_secret: "tokensecret"
"""
        with patch("builtins.open", mock_open(read_data=config_content)):
            config = load_config("test_config.yml")
            config_manager = ConfigManager(config)
            
            # オブジェクト形式でSNS設定が読み込まれることを確認
            assert isinstance(config['sns'], dict)
            assert 'mastodon' in config['sns']
            assert 'x' in config['sns']

    def test_config_manager_get_all_sns_configs_array_format(self):
        """ConfigManagerの配列形式SNS設定取得テスト"""
        config_data = {
            'sns': [
                {
                    'type': 'mastodon',
                    'name': 'mastodon-main',
                    'instance_url': 'https://mastodon.social',
                    'access_token': 'token1'
                },
                {
                    'type': 'mastodon',
                    'name': 'mstdn-jp',
                    'instance_url': 'https://mstdn.jp',
                    'access_token': 'token2'
                }
            ]
        }
        config_manager = ConfigManager(config_data)
        
        # 配列形式の設定を正しく取得できることを確認
        sns_configs = config_manager.get_all_sns_configs()
        assert isinstance(sns_configs, list)
        assert len(sns_configs) == 2

    def test_config_manager_get_all_sns_configs_object_format(self):
        """ConfigManagerのオブジェクト形式SNS設定取得テスト"""
        config_data = {
            'sns': {
                'mastodon': {
                    'instance_url': 'https://mastodon.social',
                    'access_token': 'token'
                }
            }
        }
        config_manager = ConfigManager(config_data)
        
        # オブジェクト形式の設定を正しく取得できることを確認
        sns_configs = config_manager.get_all_sns_configs()
        assert isinstance(sns_configs, dict)
        assert 'mastodon' in sns_configs


class TestMultipleAccountsPluginLoader:
    """複数アカウント対応のプラグインローダーテスト"""

    @patch('src.plugin_loader.importlib.import_module')
    def test_load_plugins_array_format_multiple_mastodon(self, mock_import):
        """配列形式での複数Mastodonアカウントプラグイン読み込みテスト"""
        # モックのMastodonプラグインクラス
        class MockMastodon:
            def __init__(self, instance_url, access_token, name=None):
                self.instance_url = instance_url
                self.access_token = access_token
                self.name = name

        mock_module = type('MockModule', (), {'Mastodon': MockMastodon})
        mock_import.return_value = mock_module

        config_data = {
            'sns': [
                {
                    'type': 'mastodon',
                    'name': 'mastodon-main',
                    'instance_url': 'https://mastodon.social',
                    'access_token': 'token1'
                },
                {
                    'type': 'mastodon',
                    'name': 'mstdn-jp',
                    'instance_url': 'https://mstdn.jp',
                    'access_token': 'token2'
                }
            ]
        }
        config_manager = ConfigManager(config_data)
        
        plugins = load_plugins(config_manager)
        
        # 複数のMastodonインスタンスが生成されることを確認
        assert len(plugins) == 2
        assert 'mastodon-main' in plugins
        assert 'mstdn-jp' in plugins
        
        # 各インスタンスの設定確認
        assert plugins['mastodon-main'].instance_url == 'https://mastodon.social'
        assert plugins['mastodon-main'].access_token == 'token1'
        assert plugins['mastodon-main'].name == 'mastodon-main'
        
        assert plugins['mstdn-jp'].instance_url == 'https://mstdn.jp'
        assert plugins['mstdn-jp'].access_token == 'token2'
        assert plugins['mstdn-jp'].name == 'mstdn-jp'

    @patch('src.plugin_loader.importlib.import_module')
    def test_load_plugins_object_format_backward_compatibility(self, mock_import):
        """オブジェクト形式プラグイン読み込み（後方互換性）テスト"""
        # モックのMastodonプラグインクラス
        class MockMastodon:
            def __init__(self, instance_url, access_token, name=None):
                self.instance_url = instance_url
                self.access_token = access_token
                self.name = name

        mock_module = type('MockModule', (), {'Mastodon': MockMastodon})
        mock_import.return_value = mock_module

        config_data = {
            'sns': {
                'mastodon': {
                    'instance_url': 'https://mastodon.social',
                    'access_token': 'token'
                }
            }
        }
        config_manager = ConfigManager(config_data)
        
        plugins = load_plugins(config_manager)
        
        # オブジェクト形式でもプラグインが生成されることを確認
        assert len(plugins) == 1
        assert 'mastodon' in plugins
        assert plugins['mastodon'].instance_url == 'https://mastodon.social'
        assert plugins['mastodon'].access_token == 'token'
        assert plugins['mastodon'].name == 'mastodon'

    @patch('src.plugin_loader.importlib.import_module')
    def test_load_plugins_mixed_sns_types(self, mock_import):
        """複数のSNS種類が混在する場合のプラグイン読み込みテスト"""
        # モックプラグインクラス群
        class MockMastodon:
            def __init__(self, instance_url, access_token, name=None):
                self.name = name

        class MockX:
            def __init__(self, consumer_key, consumer_secret, access_token, access_token_secret, name=None):
                self.name = name

        def mock_import_side_effect(module_name):
            if 'mastodon' in module_name:
                return type('MockModule', (), {'Mastodon': MockMastodon})
            elif 'x' in module_name:
                return type('MockModule', (), {'X': MockX})

        mock_import.side_effect = mock_import_side_effect

        config_data = {
            'sns': [
                {
                    'type': 'mastodon',
                    'name': 'mastodon-main',
                    'instance_url': 'https://mastodon.social',
                    'access_token': 'token1'
                },
                {
                    'type': 'x',
                    'name': 'x-personal',
                    'consumer_key': 'key1',
                    'consumer_secret': 'secret1',
                    'access_token': 'token1',
                    'access_token_secret': 'tokensecret1'
                }
            ]
        }
        config_manager = ConfigManager(config_data)
        
        plugins = load_plugins(config_manager)
        
        # 異なる種類のSNSプラグインが生成されることを確認
        assert len(plugins) == 2
        assert 'mastodon-main' in plugins
        assert 'x-personal' in plugins