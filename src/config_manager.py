import yaml
import os

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
