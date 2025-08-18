import yaml

class ConfigManager:
    def __init__(self, config):
        self.config = config

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

    def get_sns_config(self, sns_name):
        """後方互換性のためのメソッド（オブジェクト形式用）"""
        sns_configs = self.get_all_sns_configs()
        if isinstance(sns_configs, dict):
            return sns_configs.get(sns_name)
        return None

    def get_all_sns_configs(self):
        """
        SNS設定を取得します。
        配列形式とオブジェクト形式の両方をサポートします。
        
        Returns:
            list or dict: SNS設定（配列形式またはオブジェクト形式）
        """
        return self.config.get('sns', {})

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