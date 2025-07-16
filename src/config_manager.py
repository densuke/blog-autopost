import yaml

class ConfigManager:
    def __init__(self, config):
        self.config = config

    def get_feed_url(self):
        return self.config.get('blog', {}).get('feed_url')

    def get_sns_config(self, sns_name):
        return self.config.get('sns', {}).get(sns_name)

    def get_all_sns_configs(self):
        return self.config.get('sns', {})

    def get_announcement_text(self):
        return self.config.get('announcement_text', '')

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