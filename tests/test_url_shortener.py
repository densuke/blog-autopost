import pytest
from unittest.mock import patch, Mock
from src.url_shortener import URLShortener
from src.text_optimizer import TextOptimizer


class TestURLShortener:
    """URL短縮機能のテスト"""

    def test_url_shortener_init(self):
        """URLShortenerの初期化テスト"""
        shortener = URLShortener()
        assert shortener.service == "is.gd"
        assert shortener.api_url == "https://is.gd/create.php"

    @patch('src.url_shortener.requests.get')
    def test_shorten_url_success(self, mock_get):
        """URL短縮成功テスト"""
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.text = "https://is.gd/abc123"
        mock_get.return_value = mock_response

        shortener = URLShortener()
        result = shortener.shorten("https://blog.example.com/very-long-url")
        
        assert result == "https://is.gd/abc123"
        mock_get.assert_called_once_with(
            "https://is.gd/create.php",
            params={"format": "simple", "url": "https://blog.example.com/very-long-url"},
            timeout=10
        )

    @patch('src.url_shortener.requests.get')
    def test_shorten_url_failure(self, mock_get):
        """URL短縮失敗時のフォールバックテスト"""
        mock_response = Mock()
        mock_response.status_code = 400
        mock_response.text = "Error: Invalid URL"
        mock_get.return_value = mock_response

        shortener = URLShortener()
        original_url = "https://blog.example.com/test"
        result = shortener.shorten(original_url)
        
        # 失敗時は元のURLをそのまま返す
        assert result == original_url

    @patch('src.url_shortener.requests.get')
    def test_shorten_url_network_error(self, mock_get):
        """ネットワークエラー時のテスト"""
        mock_get.side_effect = Exception("Network error")

        shortener = URLShortener()
        original_url = "https://blog.example.com/test"
        result = shortener.shorten(original_url)
        
        # ネットワークエラー時も元のURLを返す
        assert result == original_url

    def test_shorten_invalid_url(self):
        """無効なURL形式のテスト"""
        shortener = URLShortener()
        invalid_url = "not-a-url"
        result = shortener.shorten(invalid_url)
        
        # 無効なURLは短縮を試行せず元のまま返す
        assert result == invalid_url


class TestTextOptimizer:
    """テキスト最適化機能のテスト"""

    def test_text_optimizer_init(self):
        """TextOptimizerの初期化テスト"""
        config = {
            'url_shortening': {'enabled': True},
            'character_limits': {'mastodon': 500, 'misskey': 3000}
        }
        optimizer = TextOptimizer(config)
        
        assert optimizer.url_shortening_enabled is True
        assert optimizer.character_limits['mastodon'] == 500
        assert optimizer.character_limits['misskey'] == 3000
        # デフォルト値の確認
        assert optimizer.character_limits['x'] == 280
        assert optimizer.character_limits['bluesky'] == 300

    def test_get_character_limit(self):
        """文字数制限取得テスト"""
        config = {'character_limits': {'mastodon': 600}}
        optimizer = TextOptimizer(config)
        
        # カスタム設定値
        assert optimizer.get_character_limit('mastodon') == 600
        # デフォルト値
        assert optimizer.get_character_limit('x') == 280
        assert optimizer.get_character_limit('bluesky') == 300
        assert optimizer.get_character_limit('misskey') == 3000

    def test_optimize_text_within_limit(self):
        """文字数制限内のテキスト最適化テスト"""
        config = {'url_shortening': {'enabled': True}}
        optimizer = TextOptimizer(config)
        
        title = "短いタイトル"
        link = "https://example.com"
        sns_type = "x"
        
        result = optimizer.optimize_text(title, link, sns_type)
        
        assert result == f"{title} {link}"

    @patch('src.text_optimizer.URLShortener')
    def test_optimize_text_with_shortening(self, mock_shortener_class):
        """URL短縮を使用したテキスト最適化テスト"""
        # URLShortenerのモック設定
        mock_shortener = Mock()
        mock_shortener.shorten.return_value = "https://is.gd/abc123"
        mock_shortener_class.return_value = mock_shortener

        config = {'url_shortening': {'enabled': True}}
        optimizer = TextOptimizer(config)
        
        # 280文字を超える長いタイトル
        long_title = "A" * 270  # 270文字のタイトル
        link = "https://example.com/very-long-path"
        sns_type = "x"  # 280文字制限
        
        result = optimizer.optimize_text(long_title, link, sns_type)
        
        # URL短縮が呼ばれることを確認
        mock_shortener.shorten.assert_called_once_with(link)
        # 短縮URLが使用されることを確認
        assert "https://is.gd/abc123" in result

    @patch('src.text_optimizer.URLShortener')
    def test_optimize_text_with_title_trimming(self, mock_shortener_class):
        """タイトルトリミングを使用したテキスト最適化テスト"""
        # URLShortenerのモック設定（短縮でも長い場合）
        mock_shortener = Mock()
        mock_shortener.shorten.return_value = "https://is.gd/abc123"
        mock_shortener_class.return_value = mock_shortener

        config = {'url_shortening': {'enabled': True}}
        optimizer = TextOptimizer(config)
        
        # 短縮後でも280文字を超えるケース
        very_long_title = "A" * 270  # 270文字のタイトル
        link = "https://example.com"
        sns_type = "x"  # 280文字制限
        
        result = optimizer.optimize_text(very_long_title, link, sns_type)
        
        # タイトルがトリミングされ、"..."が追加されることを確認
        assert "..." in result
        assert len(result) <= 280

    def test_optimize_text_shortening_disabled(self):
        """URL短縮無効時のテキスト最適化テスト"""
        config = {'url_shortening': {'enabled': False}}
        optimizer = TextOptimizer(config)
        
        long_title = "A" * 270
        link = "https://example.com"
        sns_type = "x"
        
        result = optimizer.optimize_text(long_title, link, sns_type)
        
        # URL短縮が無効な場合、タイトルトリミングのみ実行
        assert "..." in result
        assert len(result) <= 280
        # 元のリンクが使用される
        assert link in result

    def test_optimize_text_large_limit_sns(self):
        """文字数制限が大きいSNSのテスト"""
        config = {'url_shortening': {'enabled': True}}
        optimizer = TextOptimizer(config)
        
        title = "普通の長さのタイトル"
        link = "https://example.com"
        sns_type = "misskey"  # 3000文字制限
        
        result = optimizer.optimize_text(title, link, sns_type)
        
        # 制限が大きいため、最適化不要
        assert result == f"{title} {link}"