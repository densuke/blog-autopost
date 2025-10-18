import pytest
from unittest.mock import Mock, patch
from src.plugins.misskey import Misskey

def test_misskey_init():
    """Misskeyプラグインの初期化をテスト"""
    instance_url = "https://misskey.io"
    access_token = "test_token"
    
    misskey = Misskey(instance_url, access_token)
    
    assert misskey.instance_url == "https://misskey.io"
    assert misskey.access_token == "test_token"
    assert misskey.api_url == "https://misskey.io/api"

def test_misskey_init_with_trailing_slash():
    """URLの末尾にスラッシュがある場合の初期化をテスト"""
    instance_url = "https://misskey.io/"
    access_token = "test_token"
    
    misskey = Misskey(instance_url, access_token)
    
    assert misskey.instance_url == "https://misskey.io"
    assert misskey.api_url == "https://misskey.io/api"

@patch('requests.post')
def test_misskey_post_success(mock_post):
    """Misskeyへの投稿成功をテスト"""
    # モックレスポンスを設定
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = {
        'createdNote': {
            'id': 'test_note_id'
        }
    }
    mock_post.return_value = mock_response
    
    misskey = Misskey("https://misskey.io", "test_token")
    
    # 投稿実行（新しいメソッド署名: optimized_text, media_files）
    misskey.post("テストタイトル", None)
    
    # APIが正しく呼び出されたかを確認
    mock_post.assert_called_once_with(
        "https://misskey.io/api/notes/create",
        json={
            "i": "test_token",
            "text": "テストタイトル",
            "visibility": "public"
        }
    )

@patch('requests.post')
def test_misskey_post_failure(mock_post):
    """Misskeyへの投稿失敗をテスト"""
    # モックレスポンスを設定
    mock_response = Mock()
    mock_response.status_code = 400
    mock_response.text = "Bad Request"
    mock_post.return_value = mock_response
    
    misskey = Misskey("https://misskey.io", "test_token")
    
    # 投稿が失敗した場合の例外をテスト
    with pytest.raises(Exception) as exc_info:
        misskey.post("テストタイトル", "https://example.com")
    
    assert "Misskeyへの投稿に失敗しました: 400" in str(exc_info.value)
