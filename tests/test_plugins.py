import pytest
from unittest.mock import MagicMock, patch

from src.plugins.x import X
from src.plugins.misskey import Misskey

@patch('src.plugins.x.Client')
def test_x_post(mock_client):
    # Mock the Client instance and its create_tweet method
    mock_instance = MagicMock()
    mock_client.return_value = mock_instance
    mock_instance.create_tweet.return_value = MagicMock(data={'id': '12345'})

    x_plugin = X("key", "secret", "token", "token_secret")
    title = "Test Title"
    link = "http://test.com/link"
    x_plugin.post(title, link)

    # Assert that create_tweet was called with the correct text
    expected_tweet_text = f"{title} {link}"
    mock_instance.create_tweet.assert_called_once_with(text=expected_tweet_text)

@patch('requests.post')
def test_misskey_post_integration(mock_post):
    """Misskeyプラグインの統合テスト"""
    # モックレスポンスを設定
    mock_response = MagicMock()
    mock_response.status_code = 200
    mock_response.json.return_value = {
        'createdNote': {
            'id': 'test_note_id'
        }
    }
    mock_post.return_value = mock_response
    
    misskey_plugin = Misskey("https://misskey.io", "test_token")
    title = "Test Title"
    link = "http://test.com/link"
    misskey_plugin.post(title, link)
    
    # APIが正しく呼び出されたかを確認
    mock_post.assert_called_once_with(
        "https://misskey.io/api/notes/create",
        json={
            "i": "test_token",
            "text": f"{title} {link}",
            "visibility": "public"
        }
    )
