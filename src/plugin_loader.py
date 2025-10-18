import importlib

PLUGINS_DIR = "src.plugins"

def load_plugins(config_manager, sns_names=None, force_sensitive=None, dry_run=False):
    """
    SNS投稿プラグインを読み込みます。
    配列形式とオブジェクト形式の両方の設定をサポートします。

    Args:
        config_manager (ConfigManager): 設定管理オブジェクト。
        sns_names (list, optional): 読み込むSNSの識別名リスト。Noneの場合はすべて読み込む。
        force_sensitive (bool): Misskeyプラグインのセンシティブ設定を強制的に上書きする場合のフラグ
        dry_run (bool): ドライラン時はネットワーク接続を回避

    Returns:
        dict: 読み込まれたプラグインのインスタンス。キーは識別名。
    """
    plugins = {}
    sns_configs = config_manager.get_all_sns_configs()

    # フィルタリング対象の名前セットを作成
    target_names = set(sns_names) if sns_names else None

    if isinstance(sns_configs, list):
        # 配列形式の設定処理
        for sns_config in sns_configs:
            plugin_type = sns_config.get('type')
            plugin_name = sns_config.get('name', plugin_type)

            # フィルタリング
            if target_names and plugin_name not in target_names:
                continue

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

                # Misskeyプラグインの場合、force_sensitiveが指定されていれば上書き
                if plugin_type == 'misskey' and force_sensitive is not None:
                    plugin_init_config['is_sensitive'] = force_sensitive

                # 特別な初期化が必要なプラグインの処理
                if plugin_type == 'bluesky':
                    plugin_instance = plugin_class(config=config_manager.config, dry_run=dry_run, **plugin_init_config)
                elif plugin_type == 'tumblr':
                    # Tumblrプラグインの場合、configディクショナリを作成
                    config_dict = {k: v for k, v in sns_config.items() if k not in ('type', 'name', 'client_id', 'client_secret', 'access_token', 'blog_name')}
                    plugin_instance = plugin_class(
                        client_id=plugin_init_config['client_id'],
                        client_secret=plugin_init_config['client_secret'],
                        access_token=plugin_init_config['access_token'],
                        blog_name=plugin_init_config['blog_name'],
                        config=config_dict
                    )
                else:
                    plugin_instance = plugin_class(**plugin_init_config)

                # プラグインインスタンスにname属性を設定
                plugin_instance.name = plugin_name
                plugin_instance.sns_type = plugin_type

                plugins[plugin_name] = plugin_instance

            except Exception as e:
                import traceback
                print(f"プラグイン {plugin_type}（名前: {plugin_name}）の読み込み中にエラーが発生しました: {e}")
                print("詳細なエラー情報:")
                traceback.print_exc()

    elif isinstance(sns_configs, dict):
        # オブジェクト形式の設定処理（後方互換性）
        for plugin_name, plugin_config in sns_configs.items():
            # フィルタリング
            if target_names and plugin_name not in target_names:
                continue

            try:
                module = importlib.import_module(f"{PLUGINS_DIR}.{plugin_name}")
                # クラス名をプラグイン名から推測 (例: x -> X)
                class_name = plugin_name.capitalize()
                plugin_class = getattr(module, class_name)

                # Misskeyプラグインの場合、force_sensitiveが指定されていれば上書き
                if plugin_name == 'misskey' and force_sensitive is not None:
                    plugin_config = plugin_config.copy()  # 元の設定を変更しないようにコピー
                    plugin_config['is_sensitive'] = force_sensitive

                # Blueskyプラグインの場合、設定情報も渡す
                if plugin_name == 'bluesky':
                    plugin_instance = plugin_class(config=config_manager.config, dry_run=dry_run, **plugin_config)
                else:
                    plugin_instance = plugin_class(**plugin_config)

                # プラグインインスタンスにname属性を設定
                plugin_instance.name = plugin_name
                plugin_instance.sns_type = plugin_name

                plugins[plugin_name] = plugin_instance

            except Exception as e:
                print(f"プラグイン {plugin_name} の読み込み中にエラーが発生しました: {e}")

    return plugins
