import importlib

PLUGINS_DIR = "src.plugins"

def load_plugins(config_manager):
    """
    SNS投稿プラグインを読み込みます。

    Args:
        config_manager (ConfigManager): 設定管理オブジェクト。

    Returns:
        dict: 読み込まれたプラグインのインスタンス。
    """
    plugins = {}
    for plugin_name, plugin_config in config_manager.get_all_sns_configs().items():
        try:
            module = importlib.import_module(f"{PLUGINS_DIR}.{plugin_name}")
            # クラス名をプラグイン名から推測 (例: x -> X)
            class_name = plugin_name.capitalize()
            plugin_class = getattr(module, class_name)
            plugins[plugin_name] = plugin_class(**plugin_config)
        except Exception as e:
            print(f"プラグイン {plugin_name} の読み込み中にエラーが発生しました: {e}")
    return plugins
