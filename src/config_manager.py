import os
from typing import List

import yaml


class ConfigManager:
    def __init__(self, config_path):
        """
        設定管理オブジェクトを初期化します。

        Args:
            config_path (str): 設定ファイルのパス
        """
        if isinstance(config_path, str):
            # 設定ファイルのパスが渡された場合は読み込み
            self.config = load_config(config_path)
        else:
            # 辞書が直接渡された場合はそのまま使用
            self.config = config_path

    def get_feed_url(self):
        """後方互換性のためのメソッド（単一フィード用）"""
        blog_config = self.config.get('blog', {})
        if isinstance(blog_config, list):
            # 配列形式の場合は最初のフィードのURLを返す
            return blog_config[0].get('feed_url') if blog_config else None
        else:
            # オブジェクト形式の場合
            return blog_config.get('feed_url')

    def get_all_feed_configs(self):
        """
        フィード設定を取得します。
        配列形式とオブジェクト形式の両方をサポートします。
        
        Returns:
            list: フィード設定のリスト
        """
        blog_config = self.config.get('blog', {})
        if isinstance(blog_config, list):
            # 配列形式の場合
            return blog_config
        elif isinstance(blog_config, dict) and blog_config.get('feed_url'):
            # オブジェクト形式の場合、リストに変換
            return [{
                'name': 'default',
                'feed_url': blog_config.get('feed_url'),
                'image_settings': blog_config.get('image_settings')
            }]
        else:
            return []

    def get_feed_config_by_name(self, feed_name):
        """
        指定された名前のフィード設定を取得します。
        
        Args:
            feed_name (str): フィード名
            
        Returns:
            dict or None: フィード設定、見つからない場合はNone
        """
        feeds = self.get_all_feed_configs()
        for feed in feeds:
            if feed.get('name') == feed_name:
                return feed
        return None

    def get_all_sns_names(self) -> List[str]:
        """
        設定ファイルからすべてのSNSの名前を取得します。
        配列形式とオブジェクト形式の両方をサポートします。
        """
        sns_configs = self.config.get("sns", {})
        if isinstance(sns_configs, dict):
            # オブジェクト形式の場合
            return list(sns_configs.keys())
        elif isinstance(sns_configs, list):
            # 配列形式の場合、各設定の'name'フィールドを取得
            return [config.get('name') for config in sns_configs if 'name' in config]
        return []

    def get_sns_config(self, sns_name):
        """後方互換性のためのメソッド（オブジェクト形式用）"""
        sns_configs = self.get_all_sns_configs()
        if isinstance(sns_configs, dict):
            return sns_configs.get(sns_name)
        return None

    def get_all_sns_configs(self):
        """
        SNS設定を取得します。
        配列形式とオブジェクト形式の両方をサポートし、環境変数による認証情報の上書きも行います。
        
        Returns:
            list or dict: SNS設定（配列形式またはオブジェクト形式）
        """
        sns_configs = self.config.get('sns', {})

        # 環境変数による認証情報の上書きを適用
        if isinstance(sns_configs, list):
            # 配列形式の場合
            return [self._apply_env_overrides(sns_config) for sns_config in sns_configs]
        elif isinstance(sns_configs, dict):
            # オブジェクト形式の場合
            return {name: self._apply_env_overrides(config, name) for name, config in sns_configs.items()}

        return sns_configs

    def _apply_env_overrides(self, sns_config, fallback_type=None):
        """
        環境変数による認証情報の上書きを適用します
        
        Args:
            sns_config (dict): 元のSNS設定
            fallback_type (str): オブジェクト形式の場合のSNS種別（fallback）
            
        Returns:
            dict: 環境変数で上書きされた設定
        """
        config = sns_config.copy()
        sns_type = config.get('type', fallback_type)

        if not sns_type:
            return config

        # SNS種別ごとの環境変数マッピング
        env_mappings = {
            'x': {
                'consumer_key': 'X_CONSUMER_KEY',
                'consumer_secret': 'X_CONSUMER_SECRET',
                'access_token': 'X_ACCESS_TOKEN',
                'access_token_secret': 'X_ACCESS_TOKEN_SECRET'
            },
            'bluesky': {
                'identifier': 'BLUESKY_IDENTIFIER',
                'password': 'BLUESKY_PASSWORD'
            },
            'mastodon': {
                'instance_url': 'MASTODON_INSTANCE_URL',
                'access_token': 'MASTODON_ACCESS_TOKEN'
            },
            'misskey': {
                'instance_url': 'MISSKEY_INSTANCE_URL',
                'access_token': 'MISSKEY_ACCESS_TOKEN'
            },
            'threads': {
                'app_id': 'THREADS_APP_ID',
                'app_secret': 'THREADS_APP_SECRET',
                'access_token': 'THREADS_ACCESS_TOKEN'
            },
            'tumblr': {
                'client_id': 'TUMBLR_CLIENT_ID',
                'client_secret': 'TUMBLR_CLIENT_SECRET',
                'access_token': 'TUMBLR_ACCESS_TOKEN',
                'blog_name': 'TUMBLR_BLOG_NAME'
            }
        }

        # 環境変数からの上書き適用
        if sns_type in env_mappings:
            for config_key, env_key in env_mappings[sns_type].items():
                env_value = os.getenv(env_key)
                if env_value:
                    config[config_key] = env_value

        return config

    def get_announcement_text(self):
        return self.config.get('announcement_text', '')

    def get_image_settings(self, feed_name=None):
        """
        画像設定を取得します。
        
        Args:
            feed_name (str, optional): フィード名。指定しない場合は最初のフィードの設定
            
        Returns:
            dict or None: 画像設定、設定されていない場合はNone
        """
        if feed_name:
            feed_config = self.get_feed_config_by_name(feed_name)
            if feed_config:
                return feed_config.get('image_settings')
            return None
        else:
            # feed_nameが指定されていない場合は従来通りの動作
            blog_config = self.config.get('blog', {})
            if isinstance(blog_config, list):
                # 配列形式の場合は最初のフィードの設定
                return blog_config[0].get('image_settings') if blog_config else None
            else:
                # オブジェクト形式の場合
                return blog_config.get('image_settings')

    def get_web_auth_credentials(self):
        """Web UIの認証情報を取得する"""
        return self.config.get('web_auth', {})

    def get_completed_post_retention_hours(self, default: float = 12) -> float:
        """
        送信済み予約投稿を保持する時間(時間単位)を取得します。
        設定が存在しない、または不正な場合はdefaultを返します。
        """
        settings = self.config.get('scheduled_posts', {})
        raw_value = settings.get('completed_retention_hours', default)
        try:
            hours = float(raw_value)
        except (TypeError, ValueError):
            return default
        if hours <= 0:
            return default
        return hours

    def get_secret_key(self):
        """セッション管理用の秘密鍵を取得する"""
        web_auth_config = self.config.get('web_auth', {})
        secret_key = web_auth_config.get('secret_key')
        return secret_key

    def get_web_server_settings(self):
        """Webサーバーの設定を取得する"""
        server_config = self.config.get('web_server', {})
        host = server_config.get('host', '127.0.0.1')
        port = server_config.get('port', 8000)
        return {"host": host, "port": port}

    def get_default_allowed_timings(self):
        """グローバル投稿タイミング設定を取得する。

        Returns:
            投稿可能タイミング設定。設定されていない場合はNone。
            例: [['*', ['18:00', '20:00']], ['Weekday', ['09:00']]]
        """
        return self.config.get('default_allowed_timings')

    def get_allowed_timings_tolerance_minutes(self) -> int:
        """投稿タイミングの許容範囲(分)を取得する。

        Returns:
            許容時間(分)。デフォルトは5分。
        """
        return self.config.get('allowed_timings_tolerance_minutes', 5)

def load_config(config_path="config.yml"):
    """
    設定ファイルを読み込みます。

    Args:
        config_path (str): 設定ファイルのパス。

    Returns:
        dict: 設定内容。
    """
    with open(config_path, 'r', encoding='utf-8') as f:
        return yaml.safe_load(f)
