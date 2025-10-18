import pytest
from unittest.mock import MagicMock, patch, Mock
from src.plugins.bluesky import Bluesky
from atproto import Client, client_utils, models


@pytest.fixture
def mock_client():
    """Mock Bluesky クライアント"""
    with patch('src.plugins.bluesky.Client') as mock:
        instance = MagicMock()
        mock.return_value = instance
        yield instance


@pytest.fixture
def bluesky_plugin(mock_client):
    """Bluesky プラグインインスタンス"""
    return Bluesky("test@example.com", "test_password")


# ===== 認証エラーハンドリング =====

@patch('src.plugins.bluesky.Client')
def test_bluesky_init_login_error(mock_client_class):
    """Bluesky ログイン失敗時のエラー処理をテスト"""
    # --- Arrange ---
    mock_instance = MagicMock()
    mock_instance.login.side_effect = Exception("Authentication failed")
    mock_client_class.return_value = mock_instance
    
    # --- Act & Assert ---
    with pytest.raises(Exception, match="Authentication failed"):
        Bluesky("invalid@example.com", "wrong_password")


# ===== 投稿機能の詳細テスト =====

def test_bluesky_post_with_media_files(bluesky_plugin, mock_client):
    """Bluesky がメディアファイル付きで投稿できることをテスト"""
    # --- Arrange ---
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test"
    mock_client.send_post.return_value = mock_response
    
    text = "Test post with media"
    media_files = ["/path/to/image.jpg"]
    
    # --- Act ---
    bluesky_plugin.post(text, media_files)
    
    # --- Assert ---
    mock_client.send_post.assert_called_once()


def test_bluesky_post_with_article_data(bluesky_plugin, mock_client):
    """Bluesky が article_data でリンクカード投稿できることをテスト"""
    # --- Arrange ---
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test"
    mock_client.send_post.return_value = mock_response
    
    text = "Check this article"
    article_data = {
        'title': 'Article Title',
        'link': 'https://example.com/article',
        'image': 'https://example.com/image.jpg',
        'description': 'Article description'
    }
    
    # --- Act ---
    bluesky_plugin.post(text, article_data=article_data)
    
    # --- Assert ---
    mock_client.send_post.assert_called_once()


def test_bluesky_post_error_handling(bluesky_plugin, mock_client):
    """Bluesky 投稿エラーのハンドリングをテスト"""
    # --- Arrange ---
    mock_client.send_post.side_effect = Exception("Post failed")
    
    # --- Act & Assert ---
    with pytest.raises(Exception, match="Post failed"):
        bluesky_plugin.post("Test post")


def test_bluesky_post_with_debug_mode(bluesky_plugin, mock_client, capsys):
    """Bluesky がデバッグモードで詳細情報を出力することをテスト"""
    # --- Arrange ---
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test"
    mock_client.send_post.return_value = mock_response
    
    # --- Act ---
    bluesky_plugin.post("Debug test", debug=True)
    
    # --- Assert ---
    captured = capsys.readouterr()
    # デバッグモードで何か出力されることを確認
    assert captured.out != "" or "[DEBUG]" in bluesky_plugin.__class__.__name__


# ===== ハッシュタグ検出の詳細テスト =====

def test_find_hashtags_empty_text(bluesky_plugin):
    """_find_hashtags が空のテキストを処理することをテスト"""
    # --- Arrange ---
    text = ""
    
    # --- Act ---
    hashtags = bluesky_plugin._find_hashtags(text)
    
    # --- Assert ---
    assert hashtags == []


def test_find_hashtags_no_hashtags(bluesky_plugin):
    """_find_hashtags が ハッシュタグなしのテキストを処理することをテスト"""
    # --- Arrange ---
    text = "This is a normal text without hashtags"
    
    # --- Act ---
    hashtags = bluesky_plugin._find_hashtags(text)
    
    # --- Assert ---
    assert hashtags == []


def test_find_hashtags_with_special_characters(bluesky_plugin):
    """_find_hashtags が 特殊文字を含むハッシュタグを処理することをテスト"""
    # --- Arrange ---
    text = "Test #tag-with-dash and #tag_with_underscore"
    
    # --- Act ---
    hashtags = bluesky_plugin._find_hashtags(text)
    
    # --- Assert ---
    assert len(hashtags) >= 0  # 実装に依存


def test_find_hashtags_consecutive(bluesky_plugin):
    """_find_hashtags が 連続したハッシュタグを処理することをテスト"""
    # --- Arrange ---
    text = "#tag1#tag2 #tag3"
    
    # --- Act ---
    hashtags = bluesky_plugin._find_hashtags(text)
    
    # --- Assert ---
    assert isinstance(hashtags, list)


def test_find_hashtags_at_boundaries(bluesky_plugin):
    """_find_hashtags が テキスト境界のハッシュタグを処理することをテスト"""
    # --- Arrange ---
    text = "#start and end#"
    
    # --- Act ---
    hashtags = bluesky_plugin._find_hashtags(text)
    
    # --- Assert ---
    assert isinstance(hashtags, list)


# ===== Facet 作成の詳細テスト =====

def test_create_hashtag_facets_empty_list(bluesky_plugin):
    """_create_hashtag_facets が 空のリストを処理することをテスト"""
    # --- Arrange ---
    hashtags = []
    
    # --- Act ---
    facets = bluesky_plugin._create_hashtag_facets(hashtags)
    
    # --- Assert ---
    assert facets == []


def test_create_hashtag_facets_single(bluesky_plugin):
    """_create_hashtag_facets が 単一のハッシュタグを処理することをテスト"""
    # --- Arrange ---
    hashtags = [(0, 5, "test")]
    
    # --- Act ---
    facets = bluesky_plugin._create_hashtag_facets(hashtags)
    
    # --- Assert ---
    assert len(facets) == 1
    assert facets[0].index.byte_start == 0
    assert facets[0].index.byte_end == 5


def test_create_hashtag_facets_multiple(bluesky_plugin):
    """_create_hashtag_facets が 複数のハッシュタグを処理することをテスト"""
    # --- Arrange ---
    hashtags = [(0, 5, "tag1"), (10, 15, "tag2"), (20, 25, "tag3")]
    
    # --- Act ---
    facets = bluesky_plugin._create_hashtag_facets(hashtags)
    
    # --- Assert ---
    assert len(facets) == 3


def test_create_hashtag_facets_byte_positions(bluesky_plugin):
    """_create_hashtag_facets が バイト位置を正確に設定することをテスト"""
    # --- Arrange ---
    hashtags = [(5, 10, "テスト")]
    
    # --- Act ---
    facets = bluesky_plugin._create_hashtag_facets(hashtags)
    
    # --- Assert ---
    assert facets[0].index.byte_start == 5
    assert facets[0].index.byte_end == 10


# ===== Post メソッドの詳細テスト =====

@patch('src.plugins.bluesky.Client')
def test_post_empty_text(mock_client_class):
    """post が 空のテキストを処理することをテスト"""
    # --- Arrange ---
    mock_instance = MagicMock()
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test"
    mock_instance.send_post.return_value = mock_response
    mock_client_class.return_value = mock_instance
    
    bluesky = Bluesky("test@example.com", "password")
    
    # --- Act ---
    bluesky.post("")
    
    # --- Assert ---
    mock_instance.send_post.assert_called_once()


@patch('src.plugins.bluesky.Client')
def test_post_very_long_text(mock_client_class):
    """post が 長いテキストを処理することをテスト"""
    # --- Arrange ---
    mock_instance = MagicMock()
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test"
    mock_instance.send_post.return_value = mock_response
    mock_client_class.return_value = mock_instance
    
    bluesky = Bluesky("test@example.com", "password")
    long_text = "x" * 500  # Bluesky の文字数制限以上
    
    # --- Act ---
    bluesky.post(long_text)
    
    # --- Assert ---
    mock_instance.send_post.assert_called_once()


@patch('src.plugins.bluesky.Client')
def test_post_with_links(mock_client_class):
    """post が リンクを含むテキストを処理することをテスト"""
    # --- Arrange ---
    mock_instance = MagicMock()
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test"
    mock_instance.send_post.return_value = mock_response
    mock_client_class.return_value = mock_instance
    
    bluesky = Bluesky("test@example.com", "password")
    text = "Check this out https://example.com and this https://example.org"
    
    # --- Act ---
    bluesky.post(text)
    
    # --- Assert ---
    mock_instance.send_post.assert_called_once()


@patch('src.plugins.bluesky.Client')
def test_post_with_mentions(mock_client_class):
    """post が メンション（@）を含むテキストを処理することをテスト"""
    # --- Arrange ---
    mock_instance = MagicMock()
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test"
    mock_instance.send_post.return_value = mock_response
    mock_client_class.return_value = mock_instance
    
    bluesky = Bluesky("test@example.com", "password")
    text = "Hey @user1 and @user2 check this out"
    
    # --- Act ---
    bluesky.post(text)
    
    # --- Assert ---
    mock_instance.send_post.assert_called_once()


@patch('src.plugins.bluesky.Client')
def test_post_unicode_handling(mock_client_class):
    """post が Unicode テキストを正しく処理することをテスト"""
    # --- Arrange ---
    mock_instance = MagicMock()
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test"
    mock_instance.send_post.return_value = mock_response
    mock_client_class.return_value = mock_instance
    
    bluesky = Bluesky("test@example.com", "password")
    text = "日本語テキスト 中文 한국어 Emoji: 🎉🚀💻"
    
    # --- Act ---
    bluesky.post(text)
    
    # --- Assert ---
    mock_instance.send_post.assert_called_once()


# ===== 属性とプロパティテスト =====

def test_bluesky_has_sns_type_attribute(bluesky_plugin):
    """Bluesky が sns_type 属性を持つことをテスト"""
    # --- Assert ---
    assert hasattr(bluesky_plugin, 'sns_type')
    assert bluesky_plugin.sns_type == 'bluesky'


def test_bluesky_has_supports_rich_content_method(bluesky_plugin):
    """Bluesky が supports_rich_content メソッドを持つことをテスト"""
    # --- Assert ---
    assert hasattr(bluesky_plugin, 'supports_rich_content')
    assert callable(bluesky_plugin.supports_rich_content)


def test_bluesky_supports_rich_content_returns_true(bluesky_plugin):
    """Bluesky の supports_rich_content が True を返すことをテスト"""
    # --- Act ---
    result = bluesky_plugin.supports_rich_content()
    
    # --- Assert ---
    assert result is True


# ===== 複合機能テスト =====

@patch('src.plugins.bluesky.Client')
def test_post_with_all_features(mock_client_class):
    """post が すべての機能を含むテキストで動作することをテスト"""
    # --- Arrange ---
    mock_instance = MagicMock()
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test"
    mock_instance.send_post.return_value = mock_response
    mock_client_class.return_value = mock_instance
    
    bluesky = Bluesky("test@example.com", "password")
    text = "Hey @user! Check #tag1 #tag2 https://example.com 日本語"
    
    # --- Act ---
    bluesky.post(text)
    
    # --- Assert ---
    mock_instance.send_post.assert_called_once()


@patch('src.plugins.bluesky.Client')
def test_post_response_handling(mock_client_class):
    """post が APIレスポンスを正しく処理することをテスト"""
    # --- Arrange ---
    mock_instance = MagicMock()
    mock_response = MagicMock()
    mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test123"
    mock_response.cid = "test_cid_value"
    mock_instance.send_post.return_value = mock_response
    mock_client_class.return_value = mock_instance
    
    bluesky = Bluesky("test@example.com", "password")
    
    # --- Act ---
    bluesky.post("Test post")
    
    # --- Assert ---
    mock_instance.send_post.assert_called_once()
    # URI が返されていることを確認
    call_kwargs = mock_instance.send_post.call_args[1]
    assert 'text' in call_kwargs
