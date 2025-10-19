from unittest.mock import patch, MagicMock
import sys

from src.main import (
    extract_image_from_url,
    handle_list_sns,
    handle_list_feeds,
    process_media_files,
    setup_text_optimization,
    validate_and_filter_plugins,
    execute_sns_posting,
    handle_direct_text_post,
    handle_touch_rss_posted,
    main,
)


# ===== extract_image_from_url テスト =====

@patch('src.main.requests.get')
def test_extract_image_from_url_og_image_found(mock_get):
    """extract_image_from_url が OG:image メタタグから画像URL を抽出できることをテスト"""
    # --- Arrange ---
    html_content = """
    <html>
        <head>
            <meta property="og:image" content="https://example.com/image.jpg" />
        </head>
    </html>
    """
    
    mock_response = MagicMock()
    mock_response.content = html_content.encode('utf-8')
    mock_response.headers = {'content-type': 'text/html; charset=utf-8'}
    mock_get.return_value = mock_response
    
    # --- Act ---
    result = extract_image_from_url('https://example.com/article')
    
    # --- Assert ---
    assert result == 'https://example.com/image.jpg'
    mock_get.assert_called_once()


@patch('src.main.requests.get')
def test_extract_image_from_url_twitter_card_fallback(mock_get):
    """extract_image_from_url が OG:image がない場合、twitter:image にフォールバックすることをテスト"""
    # --- Arrange ---
    html_content = """
    <html>
        <head>
            <meta name="twitter:image" content="https://example.com/twitter_image.jpg" />
        </head>
    </html>
    """
    
    mock_response = MagicMock()
    mock_response.content = html_content.encode('utf-8')
    mock_response.headers = {'content-type': 'text/html; charset=utf-8'}
    mock_get.return_value = mock_response
    
    # --- Act ---
    result = extract_image_from_url('https://example.com/article')
    
    # --- Assert ---
    assert result == 'https://example.com/twitter_image.jpg'


@patch('src.main.requests.get')
def test_extract_image_from_url_no_image_found(mock_get):
    """extract_image_from_url が 画像が見つからない場合、空文字を返すことをテスト"""
    # --- Arrange ---
    html_content = "<html><head></head></html>"
    
    mock_response = MagicMock()
    mock_response.content = html_content.encode('utf-8')
    mock_response.headers = {'content-type': 'text/html; charset=utf-8'}
    mock_get.return_value = mock_response
    
    # --- Act ---
    result = extract_image_from_url('https://example.com/article')
    
    # --- Assert ---
    assert result == ''


@patch('src.main.requests.get')
def test_extract_image_from_url_relative_path_converted(mock_get):
    """extract_image_from_url が 相対パスを絶対URLに変換することをテスト"""
    # --- Arrange ---
    html_content = """
    <html>
        <head>
            <meta property="og:image" content="/images/article.jpg" />
        </head>
    </html>
    """
    
    mock_response = MagicMock()
    mock_response.content = html_content.encode('utf-8')
    mock_response.headers = {'content-type': 'text/html; charset=utf-8'}
    mock_get.return_value = mock_response
    
    # --- Act ---
    result = extract_image_from_url('https://example.com/article')
    
    # --- Assert ---
    assert result == 'https://example.com/images/article.jpg'


@patch('src.main.requests.get')
def test_extract_image_from_url_network_error(mock_get):
    """extract_image_from_url が ネットワークエラーの場合、空文字を返すことをテスト"""
    # --- Arrange ---
    mock_get.side_effect = Exception("Network error")
    
    # --- Act ---
    result = extract_image_from_url('https://example.com/article')
    
    # --- Assert ---
    assert result == ''


@patch('src.main.requests.get')
def test_extract_image_from_url_debug_mode(mock_get, capsys):
    """extract_image_from_url が デバッグモードで詳細ログを出力することをテスト"""
    # --- Arrange ---
    html_content = """
    <html>
        <head>
            <meta property="og:image" content="https://example.com/image.jpg" />
        </head>
    </html>
    """
    
    mock_response = MagicMock()
    mock_response.content = html_content.encode('utf-8')
    mock_response.headers = {'content-type': 'text/html; charset=utf-8'}
    mock_get.return_value = mock_response
    
    # --- Act ---
    result = extract_image_from_url('https://example.com/article', debug=True)
    
    # --- Assert ---
    captured = capsys.readouterr()
    assert '[DEBUG]' in captured.out
    assert result == 'https://example.com/image.jpg'



# ===== handle_list_sns テスト =====

@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_sns_array_format(mock_config_manager, mock_print):
    """handle_list_sns が 配列形式のSNS設定を正しく表示することをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_sns_configs.return_value = [
        {'type': 'x', 'name': 'x-main', 'consumer_key': 'key'},
        {'type': 'bluesky', 'name': 'bluesky-main', 'identifier': 'id', 'password': 'pwd'}
    ]
    
    # --- Act ---
    handle_list_sns(mock_cm)
    
    # --- Assert ---
    mock_print.assert_any_call("=== 登録されているSNSアカウント一覧 ===")
    # 配列形式であることを確認
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('配列形式' in str(call) for call in print_calls)


@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_sns_no_accounts(mock_config_manager, mock_print):
    """handle_list_sns が SNSアカウントがない場合、メッセージを表示することをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_sns_configs.return_value = []
    
    # --- Act ---
    handle_list_sns(mock_cm)
    
    # --- Assert ---
    mock_print.assert_any_call("SNSアカウントが設定されていません。")


@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_sns_mastodon_shows_instance(mock_config_manager, mock_print):
    """handle_list_sns が Mastodon 設定でインスタンスURLを表示することをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_sns_configs.return_value = [
        {
            'type': 'mastodon',
            'name': 'mastodon-main',
            'instance_url': 'https://mastodon.social',
            'access_token': 'token'
        }
    ]
    
    # --- Act ---
    handle_list_sns(mock_cm)
    
    # --- Assert ---
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('mastodon.social' in str(call) for call in print_calls)


# ===== handle_list_feeds テスト =====

@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_feeds_displays_feeds(mock_config_manager, mock_print):
    """handle_list_feeds が 登録されたフィード一覧を表示することをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_feed_configs.return_value = [
        {'name': 'tech-blog', 'feed_url': 'https://tech.example.com/feed'},
        {'name': 'news-feed', 'feed_url': 'https://news.example.com/feed'}
    ]
    
    # --- Act ---
    handle_list_feeds(mock_cm)
    
    # --- Assert ---
    mock_print.assert_any_call("=== 登録されているフィード一覧 ===")
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('tech-blog' in str(call) for call in print_calls)


@patch('builtins.print')
@patch('src.main.ConfigManager')
def test_handle_list_feeds_no_feeds(mock_config_manager, mock_print):
    """handle_list_feeds が フィードがない場合、メッセージを表示することをテスト"""
    # --- Arrange ---
    mock_cm = MagicMock()
    mock_cm.get_all_feed_configs.return_value = []
    
    # --- Act ---
    handle_list_feeds(mock_cm)
    
    # --- Assert ---
    mock_print.assert_any_call("フィードが設定されていません。")


# ===== process_media_files テスト =====

def test_process_media_files_empty_list():
    """process_media_files が 空のリストを渡された場合、空リストを返すことをテスト"""
    # --- Arrange ---
    args = MagicMock(debug=False)
    
    # --- Act ---
    result = process_media_files([], args)
    
    # --- Assert ---
    assert result == []


# ===== setup_text_optimization テスト =====

def test_setup_text_optimization_disabled():
    """setup_text_optimization が テキスト最適化が無効の場合、None を返すことをテスト"""
    # --- Arrange ---
    args = MagicMock(optimize=False)
    config_manager = MagicMock()
    
    # --- Act ---
    result = setup_text_optimization(args, config_manager)
    
    # --- Assert ---
    assert result is None


def test_setup_text_optimization_enabled():
    """setup_text_optimization が テキスト最適化が有効の場合、オブジェクトを返すことをテスト"""
    # --- Arrange ---
    args = MagicMock(optimize=True)
    config_manager = MagicMock()
    config_manager.config = {}
    
    # --- Act ---
    result = setup_text_optimization(args, config_manager)
    
    # --- Assert ---
    # TextOptimizer が返されることを確認
    assert result is not None


# ===== validate_and_filter_plugins テスト =====

@patch('src.main.load_plugins')
def test_validate_and_filter_plugins_no_sns_filter(mock_load_plugins):
    """validate_and_filter_plugins が SNS フィルタなしで全プラグインを返すことをテスト"""
    # --- Arrange ---
    args = MagicMock(sns=None, dry_run=False, debug=False)
    config_manager = MagicMock()
    
    mock_x_plugin = MagicMock()
    mock_x_plugin.sns_type = 'x'
    mock_bluesky_plugin = MagicMock()
    mock_bluesky_plugin.sns_type = 'bluesky'
    
    mock_load_plugins.return_value = {
        'x-main': mock_x_plugin,
        'bluesky-main': mock_bluesky_plugin
    }
    
    # --- Act ---
    result = validate_and_filter_plugins(args, config_manager)
    
    # --- Assert ---
    plugins, target_sns = result
    assert 'x-main' in plugins
    assert 'bluesky-main' in plugins
    assert target_sns is None


@patch('src.main.load_plugins')
def test_validate_and_filter_plugins_with_sns_filter(mock_load_plugins):
    """validate_and_filter_plugins が SNS フィルタで該当プラグインのみを返すことをテスト"""
    # --- Arrange ---
    args = MagicMock(sns='x', dry_run=False, debug=False)
    config_manager = MagicMock()
    
    mock_x_plugin = MagicMock()
    mock_x_plugin.sns_type = 'x'
    mock_bluesky_plugin = MagicMock()
    mock_bluesky_plugin.sns_type = 'bluesky'
    
    mock_load_plugins.return_value = {
        'x-main': mock_x_plugin,
        'bluesky-main': mock_bluesky_plugin
    }
    
    # --- Act ---
    result = validate_and_filter_plugins(args, config_manager)
    
    # --- Assert ---
    plugins, target_sns = result
    assert 'x-main' in plugins
    assert 'bluesky-main' not in plugins


@patch('src.main.load_plugins')
@patch('builtins.print')
def test_validate_and_filter_plugins_unknown_sns(mock_print, mock_load_plugins):
    """validate_and_filter_plugins が 未知のSNS指定の場合、None を返すことをテスト"""
    # --- Arrange ---
    args = MagicMock(sns='unknown_sns', dry_run=False, debug=False)
    config_manager = MagicMock()
    
    mock_load_plugins.return_value = {
        'x-main': MagicMock(sns_type='x')
    }
    
    # --- Act ---
    result = validate_and_filter_plugins(args, config_manager)
    
    # --- Assert ---
    assert result is None


# ===== execute_sns_posting テスト =====

@patch('src.main.load_plugins')
@patch('builtins.print')
def test_execute_sns_posting_dry_run(mock_print, mock_load_plugins):
    """execute_sns_posting が ドライラン時に投稿を実行しないことをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=True, optimize=False, debug=False)
    config_manager = MagicMock()
    config_manager.config = {'character_limits': {'x': 280}}
    
    plugins = {}
    mock_load_plugins.return_value = {'x-main': MagicMock(sns_type='x')}
    
    # --- Act ---
    execute_sns_posting("Test", None, plugins, None, None, args, config_manager)
    
    # --- Assert ---
    mock_print.assert_any_call("[ドライラン] 以下のSNSに投稿予定:")


@patch('builtins.print')
def test_execute_sns_posting_character_limit_warning(mock_print):
    """execute_sns_posting が 文字数制限超過時に警告を表示することをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=False, optimize=False, debug=False)
    config_manager = MagicMock()
    config_manager.config = {'character_limits': {'x': 10}}
    
    plugin = MagicMock()
    plugin.sns_type = 'x'
    plugins = {'x-main': plugin}
    
    # --- Act ---
    execute_sns_posting("This is a very long text that exceeds the limit", None, plugins, None, None, args, config_manager)
    
    # --- Assert ---
    print_calls = [str(call) for call in mock_print.call_args_list]
    assert any('警告' in str(call) for call in print_calls)


# ===== handle_direct_text_post テスト =====

@patch('src.main.execute_sns_posting')
@patch('src.main.validate_and_filter_plugins')
@patch('src.main.setup_text_optimization')
@patch('src.main.process_media_files')
def test_handle_direct_text_post_success(mock_process_media, mock_setup_text, mock_validate, mock_execute):
    """handle_direct_text_post が 直接テキスト投稿を正しく処理することをテスト"""
    # --- Arrange ---
    args = MagicMock(text="Test post", media=[], debug=False)
    config_manager = MagicMock()
    
    mock_process_media.return_value = []
    mock_setup_text.return_value = None
    mock_validate.return_value = ({'x-main': MagicMock()}, ['x'])
    
    # --- Act ---
    handle_direct_text_post(args, config_manager)
    
    # --- Assert ---
    mock_execute.assert_called_once()


# ===== handle_touch_rss_posted テスト =====

@patch('src.main.ArticleManager')
def test_handle_touch_rss_posted_dry_run(mock_article_manager):
    """handle_touch_rss_posted が ドライラン時に実行しないことをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=True)
    config_manager = MagicMock()
    
    # --- Act ---
    handle_touch_rss_posted(args, config_manager)
    
    # --- Assert ---
    # ドライラン時はArticleManagerは呼ばれない
    mock_article_manager.assert_not_called()


@patch('builtins.print')
@patch('src.main.ArticleManager')
def test_handle_touch_rss_posted_success(mock_article_manager, mock_print):
    """handle_touch_rss_posted が 成功時に結果を表示することをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=False)
    config_manager = MagicMock()
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.force_mark_all_as_posted.return_value = {
        'status': 'success',
        'processed_count': 10
    }
    
    # --- Act ---
    handle_touch_rss_posted(args, config_manager)
    
    # --- Assert ---
    mock_print.assert_any_call("処理が完了しました。処理された記事数: 10")


@patch('builtins.print')
@patch('src.main.ArticleManager')
def test_handle_touch_rss_posted_error(mock_article_manager, mock_print):
    """handle_touch_rss_posted が エラー時にエラーメッセージを表示することをテスト"""
    # --- Arrange ---
    args = MagicMock(dry_run=False)
    config_manager = MagicMock()
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.force_mark_all_as_posted.return_value = {
        'status': 'error',
        'message': 'File not found'
    }
    
    # --- Act ---
    handle_touch_rss_posted(args, config_manager)
    
    # --- Assert ---
    mock_print.assert_any_call("エラーが発生しました: File not found")


# ===== main 関数の追加テスト =====

@patch('src.main.load_config')
@patch('src.main.handle_list_sns')
def test_main_list_sns_option(mock_handle_list_sns, mock_load_config):
    """main が --list-sns オプションで handle_list_sns を呼び出すことをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {}
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--list-sns']):
        main()
    
    # --- Assert ---
    mock_handle_list_sns.assert_called_once()


@patch('src.main.load_config')
@patch('src.main.handle_list_feeds')
def test_main_list_feeds_option(mock_handle_list_feeds, mock_load_config):
    """main が --list-feeds オプションで handle_list_feeds を呼び出すことをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {}
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--list-feeds']):
        main()
    
    # --- Assert ---
    mock_handle_list_feeds.assert_called_once()


@patch('src.main.load_config')
@patch('src.main.handle_direct_text_post')
def test_main_text_option(mock_handle_text_post, mock_load_config):
    """main が --text オプションで handle_direct_text_post を呼び出すことをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {}
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--text', 'Test post']):
        main()
    
    # --- Assert ---
    mock_handle_text_post.assert_called_once()


@patch('src.main.load_config')
@patch('src.main.ConfigManager')
@patch('src.main.MultiArticleManager')
def test_main_multi_feed_support(mock_multi_am, mock_config_manager_class, mock_load_config):
    """main が 複数フィード対応で MultiArticleManager を使用することをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {
        'feeds': [
            {'name': 'feed1', 'feed_url': 'http://feed1.com'},
            {'name': 'feed2', 'feed_url': 'http://feed2.com'}
        ]
    }
    
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
    mock_multi_am.assert_called_once()


# ===== 統合テスト =====

@patch('src.main.load_config')
@patch('src.main.ArticleManager')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_end_to_end_new_article_posting(mock_print, mock_load_plugins, mock_article_manager, mock_load_config):
    """main のエンドツーエンドフロー: 新着記事検出から投稿までをテスト"""
    # --- Arrange ---
    mock_load_config.return_value = {
        'blog': {'feed_url': 'http://test.com/feed'},
        'announcement_text': 'New Post:'
    }
    
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_latest_articles.return_value = [
        {'title': 'Article 1', 'link': 'http://test.com/1'}
    ]
    mock_am_instance.load_saved_articles.return_value = []
    mock_am_instance.get_new_articles.return_value = [
        {'title': 'Article 1', 'link': 'http://test.com/1'}
    ]
    mock_am_instance.create_post_text.return_value = "New Post: Article 1 http://test.com/1"
    
    mock_plugin = MagicMock()
    mock_plugin.supports_rich_content.return_value = False
    mock_load_plugins.return_value = {'x-main': mock_plugin}
    
    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py']):
        main()
    
    # --- Assert ---
    mock_plugin.post.assert_called_once()
    mock_am_instance.save_articles.assert_called_once()
