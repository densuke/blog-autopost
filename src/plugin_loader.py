import importlib

PLUGINS_DIR = "src.plugins"

def load_plugins(config_manager):
    """
    SNS投稿プラグインを読み込みます。
    配列形式とオブジェクト形式の両方の設定をサポートします。

    Args:
        config_manager (ConfigManager): 設定管理オブジェクト。

    Returns:
        dict: 読み込まれたプラグインのインスタンス。キーは識別名。
    """
    plugins = {}
    sns_configs = config_manager.get_all_sns_configs()
    
    if isinstance(sns_configs, list):
        # 配列形式の設定処理
        for sns_config in sns_configs:
            plugin_type = sns_config.get('type')
            plugin_name = sns_config.get('name', plugin_type)
            
            if not plugin_type:
                print(f"SNS設定でtypeが指定されていません: {sns_config}")
                continue
                
            try:
                # プラグインモジュールを読み込み
                module = importlib.import_module(f"{PLUGINS_DIR}.{plugin_type}")
                # クラス名をプラグイン名から推測 (例: x -> X)
                class_name = plugin_type.capitalize()
                plugin_class = getattr(module, class_name)
                
                # name以外の設定をプラグインコンストラクタに渡す
                plugin_init_config = {k: v for k, v in sns_config.items() if k not in ('type', 'name')}
                plugin_instance = plugin_class(**plugin_init_config)
                
                # プラグインインスタンスにname属性を設定
                plugin_instance.name = plugin_name
                
                plugins[plugin_name] = plugin_instance
                
            except Exception as e:
                print(f"プラグイン {plugin_type}（名前: {plugin_name}）の読み込み中にエラーが発生しました: {e}")
                
    elif isinstance(sns_configs, dict):
        # オブジェクト形式の設定処理（後方互換性）
        for plugin_name, plugin_config in sns_configs.items():
            try:
                module = importlib.import_module(f"{PLUGINS_DIR}.{plugin_name}")
                # クラス名をプラグイン名から推測 (例: x -> X)
                class_name = plugin_name.capitalize()
                plugin_class = getattr(module, class_name)
                plugin_instance = plugin_class(**plugin_config)
                
                # プラグインインスタンスにname属性を設定
                plugin_instance.name = plugin_name
                
                plugins[plugin_name] = plugin_instance
                
            except Exception as e:
                print(f"プラグイン {plugin_name} の読み込み中にエラーが発生しました: {e}")
    
    return plugins
