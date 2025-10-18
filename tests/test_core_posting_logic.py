import pytest
from unittest.mock import MagicMock, patch
from src.web.core_posting_logic import CorePostingLogic


@pytest.fixture
def mock_config_manager():
    """ConfigManager のモック"""
    mock_cm = MagicMock()
    mock_cm.config = {
        'character_limits': {
            'x': 280,
            'bluesky': 300,
            'mastodon': 500,
            'misskey': 3000
        }
    }
    return mock_cm


@pytest.fixture
def core_posting_logic(mock_config_manager):
    """CorePostingLogic インスタンス"""
    return CorePostingLogic(mock_config_manager)


def test_post_to_sns_success(core_posting_logic):
    """post_to_sns が正常に投稿できることをテスト"""
    # --- Arrange ---
    with patch('src.web.core_posting_logic.load_plugins') as mock_load_plugins:
        mock_plugin = MagicMock()
        mock_plugin.sns_type = 'x'
        mock_plugin.supports_rich_content.return_value = False

        mock_load_plugins.return_value = {'x-main': mock_plugin}

        content = "Test post content"

        # --- Act ---
        result = core_posting_logic.post_to_sns(content)

        # --- Assert ---
        assert result['success'] is True
        assert 'x-main' in result['results']
        assert result['results']['x-main'] == 'success'
        mock_plugin.post.assert_called_once()


def test_post_to_sns_no_plugins(core_posting_logic):
    """post_to_sns が使用可能なプラグインがない場合を処理することをテスト"""
    # --- Arrange ---
    with patch('src.web.core_posting_logic.load_plugins') as mock_load_plugins:
        mock_load_plugins.return_value = {}

        content = "Test post content"

        # --- Act ---
        result = core_posting_logic.post_to_sns(content)

        # --- Assert ---
        assert result['success'] is False
        assert 'general' in result['errors']


def test_post_to_sns_with_target_sns(core_posting_logic):
    """post_to_sns が対象SNS指定で正しく動作することをテスト"""
    # --- Arrange ---
    with patch('src.web.core_posting_logic.load_plugins') as mock_load_plugins:
        mock_x_plugin = MagicMock()
        mock_x_plugin.sns_type = 'x'
        mock_x_plugin.supports_rich_content.return_value = False

        mock_bluesky_plugin = MagicMock()
        mock_bluesky_plugin.sns_type = 'bluesky'
        mock_bluesky_plugin.supports_rich_content.return_value = False

        mock_load_plugins.return_value = {
            'x-main': mock_x_plugin,
            'bluesky-main': mock_bluesky_plugin
        }

        content = "Test post content"

        # --- Act ---
        result = core_posting_logic.post_to_sns(content, target_sns=['x'])

        # --- Assert ---
        assert result['success'] is True
        assert 'x-main' in result['results']
        assert 'bluesky-main' not in result['results']
        mock_x_plugin.post.assert_called_once()
        mock_bluesky_plugin.post.assert_not_called()


def test_post_to_sns_with_media_files(core_posting_logic):
    """post_to_sns がメディアファイルを正しく渡すことをテスト"""
    # --- Arrange ---
    with patch('src.web.core_posting_logic.load_plugins') as mock_load_plugins:
        mock_plugin = MagicMock()
        mock_plugin.sns_type = 'x'
        mock_plugin.supports_rich_content.return_value = False

        mock_load_plugins.return_value = {'x-main': mock_plugin}

        content = "Test post with media"
        media_files = ['/path/to/image.jpg', '/path/to/video.mp4']

        # --- Act ---
        result = core_posting_logic.post_to_sns(content, media_files=media_files)

        # --- Assert ---
        assert result['success'] is True
        mock_plugin.post.assert_called_once()

        # media_files が渡されていることを確認
        call_args = mock_plugin.post.call_args
        assert call_args[0][1] == media_files  # 2番目の引数がmedia_files


def test_post_to_sns_with_url_extraction(core_posting_logic):
    """post_to_sns がURLを抽出してarticle_dataを作成することをテスト"""
    # --- Arrange ---
    with patch('src.web.core_posting_logic.load_plugins') as mock_load_plugins:
        with patch('src.web.core_posting_logic.extract_image_from_url') as mock_extract_image:
            mock_plugin = MagicMock()
            mock_plugin.sns_type = 'bluesky'
            mock_plugin.supports_rich_content.return_value = True

            mock_load_plugins.return_value = {'bluesky-main': mock_plugin}
            mock_extract_image.return_value = 'http://example.com/image.jpg'

            content = "Check this article https://example.com/blog/post"

            # --- Act ---
            result = core_posting_logic.post_to_sns(content)

            # --- Assert ---
            assert result['success'] is True
            mock_plugin.post.assert_called_once()

            # article_data が渡されていることを確認
            call_kwargs = mock_plugin.post.call_args[1]
            assert 'article_data' in call_kwargs
            assert call_kwargs['article_data']['link'] == 'https://example.com/blog/post'


def test_post_to_sns_with_text_optimization(core_posting_logic):
    """post_to_sns がテキスト最適化を適用することをテスト"""
    # --- Arrange ---
    with patch('src.web.core_posting_logic.load_plugins') as mock_load_plugins:
        with patch('src.web.core_posting_logic.TextOptimizer') as mock_text_optimizer_class:
            mock_plugin = MagicMock()
            mock_plugin.sns_type = 'x'
            mock_plugin.supports_rich_content.return_value = False

            mock_load_plugins.return_value = {'x-main': mock_plugin}

            mock_optimizer = MagicMock()
            mock_optimizer.optimize_text.return_value = "Optimized text"
            mock_text_optimizer_class.return_value = mock_optimizer

            content = "Long post content with URL https://example.com/blog/post"

            # --- Act ---
            result = core_posting_logic.post_to_sns(content, optimize=True)

            # --- Assert ---
            assert result['success'] is True
            mock_optimizer.optimize_text.assert_called_once()


def test_post_to_sns_plugin_error_handling(core_posting_logic):
    """post_to_sns がプラグインエラーを正しく処理することをテスト"""
    # --- Arrange ---
    with patch('src.web.core_posting_logic.load_plugins') as mock_load_plugins:
        mock_x_plugin = MagicMock()
        mock_x_plugin.sns_type = 'x'
        mock_x_plugin.supports_rich_content.return_value = False
        mock_x_plugin.post.side_effect = Exception("API Error")

        mock_bluesky_plugin = MagicMock()
        mock_bluesky_plugin.sns_type = 'bluesky'
        mock_bluesky_plugin.supports_rich_content.return_value = False

        mock_load_plugins.return_value = {
            'x-main': mock_x_plugin,
            'bluesky-main': mock_bluesky_plugin
        }

        content = "Test post content"

        # --- Act ---
        result = core_posting_logic.post_to_sns(content)

        # --- Assert ---
        assert 'x-main' in result['errors']
        assert result['errors']['x-main'] == "API Error"
        assert 'bluesky-main' in result['results']
        assert result['results']['bluesky-main'] == 'success'


def test_post_to_sns_debug_mode(core_posting_logic):
    """post_to_sns がデバッグモードで詳細ログを出力することをテスト"""
    # --- Arrange ---
    with patch('src.web.core_posting_logic.load_plugins') as mock_load_plugins:
        mock_plugin = MagicMock()
        mock_plugin.sns_type = 'x'
        mock_plugin.supports_rich_content.return_value = False

        mock_load_plugins.return_value = {'x-main': mock_plugin}

        content = "Test post content"

        # --- Act ---
        result = core_posting_logic.post_to_sns(content, debug=True)

        # --- Assert ---
        assert result['success'] is True
        mock_plugin.post.assert_called_once_with(
            'Test post content',
            None,
            article_data=None,
            debug=True
        )


def test_post_to_sns_general_exception(core_posting_logic):
    """post_to_sns が一般例外を処理することをテスト"""
    # --- Arrange ---
    with patch('src.web.core_posting_logic.load_plugins') as mock_load_plugins:
        mock_load_plugins.side_effect = Exception("Config Error")

        content = "Test post content"

        # --- Act ---
        result = core_posting_logic.post_to_sns(content)

        # --- Assert ---
        assert result['success'] is False
        assert 'general' in result['errors']
        assert "Config Error" in result['errors']['general']
