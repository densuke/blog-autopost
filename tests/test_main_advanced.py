from unittest.mock import patch, MagicMock
import sys

from src.main import (
    handle_list_sns,
    execute_sns_posting,
    main,
)


# ===== handle_list_sns の詳細テスト =====

@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_sns_dict_format_legacy(mock_config_manager, mock_print):
    """handle_list_sns が オブジェクト形式（従来形式）のSNS設定を正しく表示することをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_sns_configs.return_value = {
        'x-main': {'consumer_key': 'key'},
        'bluesky-main': {'identifier': 'id', 'password': 'pwd'}
    }
    
    # --- Act ---
    handle_list_sns(mock_cm)
    
    # --- Assert ---
    mock_print.assert_any_call("=== 登録されているSNSアカウント一覧 ===")
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('オブジェクト形式' in str(call) for call in print_calls)


@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_sns_x_with_credentials(mock_config_manager, mock_print):
    """handle_list_sns が X 認証情報の状態を正しく表示することをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_sns_configs.return_value = [
        {
            'type': 'x',
            'name': 'x-main',
            'consumer_key': 'key',
            'consumer_secret': 'secret',
            'access_token': 'token',
            'access_token_secret': 'token_secret'
        }
    ]
    
    # --- Act ---
    handle_list_sns(mock_cm)
    
    # --- Assert ---
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('設定済み' in str(call) for call in print_calls)


@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_sns_x_incomplete_credentials(mock_config_manager, mock_print):
    """handle_list_sns が X 認証情報不完全を正しく表示することをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_sns_configs.return_value = [
        {
            'type': 'x',
            'name': 'x-main',
            'consumer_key': 'key'
            # 他の必須フィールドが足りない
        }
    ]
    
    # --- Act ---
    handle_list_sns(mock_cm)
    
    # --- Assert ---
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('不完全' in str(call) for call in print_calls)


@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_sns_bluesky_credentials_check(mock_config_manager, mock_print):
    """handle_list_sns が Bluesky 認証情報をチェックすることをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_sns_configs.return_value = [
        {
            'type': 'bluesky',
            'name': 'bluesky-main',
            'identifier': 'user@example.com',
            'password': 'password'
        }
    ]
    
    # --- Act ---
    handle_list_sns(mock_cm)
    
    # --- Assert ---
    mock_print.assert_any_call("=== 登録されているSNSアカウント一覧 ===")


@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_sns_misskey_shows_instance_url(mock_config_manager, mock_print):
    """handle_list_sns が Misskey インスタンスURLを表示することをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_sns_configs.return_value = [
        {
            'type': 'misskey',
            'name': 'misskey-main',
            'instance_url': 'https://misskey.io',
            'access_token': 'token'
        }
    ]
    
    # --- Act ---
    handle_list_sns(mock_cm)
    
    # --- Assert ---
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('misskey.io' in str(call) for call in print_calls)


# ===== process_media_files の詳細テスト =====

@patch('src.main.load_plugins')
@patch('builtins.print')
def test_execute_sns_posting_with_rich_content_support(mock_print, mock_load_plugins):
    """execute_sns_posting が リッチコンテンツ対応プラグインに article_data を渡すことをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=False, optimize=False, debug=False)
    config_manager = MagicMock()
    config_manager.config = {'character_limits': {'bluesky': 300}}
    
    plugin = MagicMock()
    plugin.sns_type = 'bluesky'
    plugin.supports_rich_content.return_value = True
    plugins = {'bluesky-main': plugin}
    
    text = "Test article https://example.com/article"
    
    # --- Act ---
    execute_sns_posting(text, None, plugins, None, None, args, config_manager)
    
    # --- Assert ---
    plugin.post.assert_called_once()
    call_kwargs = plugin.post.call_args[1]
    assert 'article_data' in call_kwargs


@patch('src.main.extract_image_from_url')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_execute_sns_posting_extracts_article_image(mock_print, mock_load_plugins, mock_extract_image):
    """execute_sns_posting が URLから画像を抽出することをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=False, optimize=False, debug=False)
    config_manager = MagicMock()
    config_manager.config = {'character_limits': {'bluesky': 300}}
    
    plugin = MagicMock()
    plugin.sns_type = 'bluesky'
    plugin.supports_rich_content.return_value = True
    plugins = {'bluesky-main': plugin}
    
    text = "Test article https://example.com/article"
    mock_extract_image.return_value = 'https://example.com/image.jpg'
    
    # --- Act ---
    execute_sns_posting(text, None, plugins, None, None, args, config_manager)
    
    # --- Assert ---
    # extract_image_from_url は複数回呼ばれる可能性がある
    assert mock_extract_image.call_count >= 1
    call_kwargs = plugin.post.call_args[1]
    article_data = call_kwargs['article_data']
    assert article_data['image'] == 'https://example.com/image.jpg'


@patch('builtins.print')
def test_execute_sns_posting_plugin_post_error_handling(mock_print):
    """execute_sns_posting が プラグインの投稿エラーをキャッチして処理することをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=False, optimize=False, debug=False)
    config_manager = MagicMock()
    config_manager.config = {'character_limits': {'x': 280}}
    
    plugin = MagicMock()
    plugin.sns_type = 'x'
    plugin.post.side_effect = Exception("API Error: Rate limit exceeded")
    plugins = {'x-main': plugin}
    
    text = "Test post"
    
    # --- Act ---
    execute_sns_posting(text, None, plugins, None, None, args, config_manager)
    
    # --- Assert ---
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('投稿失敗' in str(call) for call in print_calls)


@patch('src.main.load_plugins')
@patch('builtins.print')
def test_execute_sns_posting_with_text_optimization(mock_print, mock_load_plugins):
    """execute_sns_posting が --optimize オプション時にテキストを最適化することをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=False, optimize=True, debug=False)
    config_manager = MagicMock()
    config_manager.config = {'character_limits': {'x': 280}}
    
    plugin = MagicMock()
    plugin.sns_type = 'x'
    plugin.supports_rich_content.return_value = False
    plugins = {'x-main': plugin}
    
    mock_optimizer = MagicMock()
    mock_optimizer.optimize_text.return_value = "Optimized: Test https://example.com"
    
    text = "Test post with URL https://example.com/very/long/path/to/article"
    
    # --- Act ---
    execute_sns_posting(text, None, plugins, None, mock_optimizer, args, config_manager)
    
    # --- Assert ---
    plugin.post.assert_called_once()


@patch('src.main.load_plugins')
@patch('builtins.print')
def test_execute_sns_posting_debug_mode_output(mock_print, mock_load_plugins):
    """execute_sns_posting が デバッグモードで詳細ログを出力することをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=False, optimize=False, debug=True)
    config_manager = MagicMock()
    config_manager.config = {'character_limits': {'x': 280}}
    
    plugin = MagicMock()
    plugin.sns_type = 'x'
    plugins = {'x-main': plugin}
    
    text = "Test post"
    
    # --- Act ---
    execute_sns_posting(text, None, plugins, None, None, args, config_manager)
    
    # --- Assert ---
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('投稿中' in str(call) for call in print_calls)


# ===== main() 複数フィード処理テスト =====

@patch('src.main.load_config')
@patch('src.main.ConfigManager')
@patch('src.main.MultiArticleManager')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_multi_feed_new_articles_found(mock_print, mock_load_plugins, mock_multi_am, mock_config_manager_class, mock_load_config):
    """main が 複数フィード対応で新着記事を検出・投稿することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {}
    
    mock_config_manager = MagicMock()
    mock_config_manager_class.return_value = mock_config_manager
    mock_config_manager.get_all_feed_configs.return_value = [
        {'name': 'feed1', 'feed_url': 'http://feed1.com'},
        {'name': 'feed2', 'feed_url': 'http://feed2.com'}
    ]
    
    mock_multi_am_instance = MagicMock()
    mock_multi_am.return_value = mock_multi_am_instance
    
    mock_multi_am_instance.get_all_new_articles.return_value = {
        'feed1': {
            'articles': [{'title': 'Article 1', 'link': 'http://feed1.com/1'}],
            'feed_config': {'name': 'feed1', 'feed_url': 'http://feed1.com'}
        }
    }
    
    mock_plugin = MagicMock()
    mock_plugin.sns_type = 'x'
    mock_load_plugins.return_value = {'x-main': mock_plugin}
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py']):
        main()
    
    # --- Assert ---
    mock_multi_am_instance.get_all_new_articles.assert_called_once()
    mock_multi_am_instance.save_all_articles.assert_called_once()


@patch('src.main.load_config')
@patch('src.main.ConfigManager')
@patch('src.main.MultiArticleManager')
@patch('builtins.print')
def test_main_multi_feed_no_new_articles(mock_print, mock_multi_am, mock_config_manager_class, mock_load_config):
    """main が 複数フィード対応で新着記事がない場合を処理することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {}
    
    mock_config_manager = MagicMock()
    mock_config_manager_class.return_value = mock_config_manager
    mock_config_manager.get_all_feed_configs.return_value = [
        {'name': 'feed1', 'feed_url': 'http://feed1.com'},
        {'name': 'feed2', 'feed_url': 'http://feed2.com'}
    ]
    
    mock_multi_am_instance = MagicMock()
    mock_multi_am.return_value = mock_multi_am_instance
    mock_multi_am_instance.get_all_new_articles.return_value = None
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py']):
        main()
    
    # --- Assert ---
    mock_print.assert_any_call("新しい記事はありませんでした。")


@patch('src.main.load_config')
@patch('src.main.ConfigManager')
@patch('src.main.MultiArticleManager')
@patch('src.main.load_plugins')
def test_main_multi_feed_with_sns_filter(mock_load_plugins, mock_multi_am, mock_config_manager_class, mock_load_config):
    """main が 複数フィード対応で --sns フィルタが機能することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {}
    
    mock_config_manager = MagicMock()
    mock_config_manager_class.return_value = mock_config_manager
    mock_config_manager.get_all_feed_configs.return_value = [
        {'name': 'feed1', 'feed_url': 'http://feed1.com'},
        {'name': 'feed2', 'feed_url': 'http://feed2.com'}
    ]
    
    mock_multi_am_instance = MagicMock()
    mock_multi_am.return_value = mock_multi_am_instance
    mock_multi_am_instance.get_all_new_articles.return_value = {
        'feed1': {
            'articles': [{'title': 'Article 1', 'link': 'http://feed1.com/1'}],
            'feed_config': {'name': 'feed1'}
        }
    }
    
    mock_x_plugin = MagicMock()
    mock_x_plugin.sns_type = 'x'
    mock_bluesky_plugin = MagicMock()
    mock_bluesky_plugin.sns_type = 'bluesky'
    
    mock_load_plugins.return_value = {
        'x-main': mock_x_plugin,
        'bluesky-main': mock_bluesky_plugin
    }
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--sns', 'x']):
        main()
    
    # --- Assert ---
    mock_x_plugin.post.assert_called_once()
    mock_bluesky_plugin.post.assert_not_called()


@patch('src.main.load_config')
@patch('src.main.ConfigManager')
@patch('src.main.MultiArticleManager')
@patch('src.main.load_plugins')
def test_main_multi_feed_with_feed_filter(mock_load_plugins, mock_multi_am, mock_config_manager_class, mock_load_config):
    """main が --feed フィルタで特定フィードのみを処理することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {}
    
    mock_config_manager = MagicMock()
    mock_config_manager_class.return_value = mock_config_manager
    mock_config_manager.get_all_feed_configs.return_value = [
        {'name': 'feed1', 'feed_url': 'http://feed1.com'},
        {'name': 'feed2', 'feed_url': 'http://feed2.com'}
    ]
    
    mock_multi_am_instance = MagicMock()
    mock_multi_am.return_value = mock_multi_am_instance
    mock_multi_am_instance.get_all_new_articles.return_value = None
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--feed', 'feed1']):
        main()
    
    # --- Assert ---
    call_kwargs = mock_multi_am_instance.get_all_new_articles.call_args[1]
    assert call_kwargs.get('feed_filter') == ['feed1']


# ===== main() 単一フィード処理の詳細テスト =====

@patch('src.main.load_config')
@patch('src.main.ArticleManager')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_single_feed_with_debug(mock_print, mock_load_plugins, mock_article_manager, mock_load_config):
    """main が --debug オプションでデバッグ情報を出力することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {
        'blog': {'feed_url': 'http://test.com/feed'}
    }
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_new_articles.return_value = []
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--debug']):
        main()
    
    # --- Assert ---
    mock_am_instance.get_latest_articles.assert_called_once_with(True)


@patch('src.main.load_config')
@patch('src.main.ArticleManager')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_single_feed_with_limit(mock_print, mock_load_plugins, mock_article_manager, mock_load_config):
    """main が --limit オプションで記事数制限が機能することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {
        'blog': {'feed_url': 'http://test.com/feed'}
    }
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_new_articles.return_value = []
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--limit', '5']):
        main()
    
    # --- Assert ---
    call_kwargs = mock_am_instance.get_new_articles.call_args[0]
    assert call_kwargs[3] == 5  # 4番目の引数が limit


@patch('src.main.load_config')
@patch('src.main.ArticleManager')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_single_feed_sns_filter_not_found(mock_print, mock_load_plugins, mock_article_manager, mock_load_config):
    """main が 指定されたSNSが見つからない場合、エラーメッセージを表示することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {
        'blog': {'feed_url': 'http://test.com/feed'}
    }
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_new_articles.return_value = [
        {'title': 'Article', 'link': 'http://test.com/1'}
    ]
    
    mock_load_plugins.return_value = {'x-main': MagicMock(sns_type='x')}
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--sns', 'unknown_sns']):
        main()
    
    # --- Assert ---
    mock_print.assert_any_call("指定されたSNS (unknown_sns) が見つかりませんでした。")


@patch('src.main.load_config')
@patch('src.main.ArticleManager')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_single_feed_plugin_post_error(mock_print, mock_load_plugins, mock_article_manager, mock_load_config):
    """main が プラグイン投稿エラーをキャッチして処理することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {
        'blog': {'feed_url': 'http://test.com/feed'}
    }
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_new_articles.return_value = [
        {'title': 'Article', 'link': 'http://test.com/1'}
    ]
    mock_am_instance.create_post_text.return_value = "Test post"
    
    mock_plugin = MagicMock()
    mock_plugin.sns_type = 'x'
    mock_plugin.post.side_effect = Exception("API Error")
    mock_load_plugins.return_value = {'x-main': mock_plugin}
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py']):
        main()
    
    # --- Assert ---
    mock_print.assert_any_call("x-mainへの投稿中にエラー: API Error")


@patch('src.main.load_config')
@patch('src.main.ArticleManager')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_single_feed_rich_content_support(mock_print, mock_load_plugins, mock_article_manager, mock_load_config):
    """main が リッチコンテンツ対応プラグインに article_data を渡すことをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {
        'blog': {'feed_url': 'http://test.com/feed'}
    }
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_new_articles.return_value = [
        {'title': 'Article', 'link': 'http://test.com/1'}
    ]
    mock_am_instance.create_post_text.return_value = "Article http://test.com/1"
    
    mock_plugin = MagicMock()
    mock_plugin.sns_type = 'bluesky'
    mock_plugin.supports_rich_content.return_value = True
    mock_load_plugins.return_value = {'bluesky-main': mock_plugin}
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py']):
        main()
    
    # --- Assert ---
    mock_plugin.post.assert_called_once()
    call_kwargs = mock_plugin.post.call_args[1]
    assert 'article_data' in call_kwargs


# ===== エラーハンドリングとエッジケース =====

@patch('src.main.load_config')
@patch('builtins.print')
def test_main_config_error_handling(mock_print, mock_load_config):
    """main が 設定読み込みエラーをハンドルすることをテスト"""
    # --- Arrange ---
    mock_load_config.side_effect = FileNotFoundError("config.yml not found")
    
    # --- Act ---
    try:
        with patch.object(sys, 'argv', ['src/main.py']):
            main()
    except FileNotFoundError:
        pass  # 例外は予期される
    
    # --- Assert ---
    mock_load_config.assert_called_once()


@patch('src.main.load_config')
@patch('src.main.ConfigManager')
@patch('src.main.ArticleManager')
@patch('builtins.print')
def test_main_single_feed_dry_run_no_save(mock_print, mock_article_manager, mock_config_manager_class, mock_load_config):
    """main が --dry-run で記事を保存しないことをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {'blog': {'feed_url': 'http://test.com/feed'}}
    
    mock_config_manager = MagicMock()
    mock_config_manager_class.return_value = mock_config_manager
    mock_config_manager.get_all_feed_configs.return_value = [
        {'name': 'default', 'feed_url': 'http://test.com/feed'}
    ]
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_new_articles.return_value = [
        {'title': 'Article', 'link': 'http://test.com/1'}
    ]
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--dry-run']):
        main()
    
    # --- Assert ---
    mock_am_instance.save_articles.assert_not_called()


@patch('src.main.load_config')
@patch('src.main.ConfigManager')
@patch('src.main.ArticleManager')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_single_feed_posts_to_all_plugins_by_default(mock_print, mock_load_plugins, mock_article_manager, mock_config_manager_class, mock_load_config):
    """main が SNS フィルタなしで全プラグインに投稿することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {'blog': {'feed_url': 'http://test.com/feed'}}
    
    mock_config_manager = MagicMock()
    mock_config_manager_class.return_value = mock_config_manager
    mock_config_manager.get_all_feed_configs.return_value = [
        {'name': 'default', 'feed_url': 'http://test.com/feed'}
    ]
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_new_articles.return_value = [
        {'title': 'Article', 'link': 'http://test.com/1'}
    ]
    mock_am_instance.create_post_text.return_value = "Article http://test.com/1"
    
    mock_x_plugin = MagicMock()
    mock_x_plugin.sns_type = 'x'
    mock_bluesky_plugin = MagicMock()
    mock_bluesky_plugin.sns_type = 'bluesky'
    
    mock_load_plugins.return_value = {
        'x-main': mock_x_plugin,
        'bluesky-main': mock_bluesky_plugin
    }
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py']):
        main()
    
    # --- Assert ---
    mock_x_plugin.post.assert_called_once()
    mock_bluesky_plugin.post.assert_called_once()
