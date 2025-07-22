import yaml

class ConfigManager:
    def __init__(self, config):
        self.config = config

    def get_feed_url(self):
        return self.config.get('blog', {}).get('feed_url')

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
    
    def get_image_settings(self):
        """
        画像設定を取得します。
        
        Returns:
            dict or None: 画像設定、設定されていない場合はNone
        """
        return self.config.get('blog', {}).get('image_settings')

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