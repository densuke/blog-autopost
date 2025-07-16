from unittest.mock import MagicMock, patch

@patch('requests.post')
def test_mastodon_post(mock_post):
    """Mastodonプラグインの投稿テスト"""
    from src.plugins.mastodon import Mastodon
    # モックレスポンスを設定
    mock_response = MagicMock()
    mock_response.status_code = 200
    mock_response.json.return_value = {"id": "mastodon_test_id"}
    mock_post.return_value = mock_response

    mastodon_plugin = Mastodon("https://mastodon.example", "test_token")
    title = "Test Title"
    link = "http://test.com/link"
    mastodon_plugin.post(title, link)

    # APIが正しく呼び出されたかを確認
    mock_post.assert_called_once_with(
        "https://mastodon.example/api/v1/statuses",
        headers={"Authorization": "Bearer test_token"},
        json={"status": f"{title} {link}"}
    )

@patch('requests.post')
def test_mastodon_post_error(mock_post):
    """Mastodonプラグインの投稿エラー時テスト"""
    from src.plugins.mastodon import Mastodon
    mock_response = MagicMock()
    mock_response.status_code = 400
    mock_response.text = "Bad Request"
    mock_response.json.return_value = {}
    mock_post.return_value = mock_response

    mastodon_plugin = Mastodon("https://mastodon.example", "test_token")
    mastodon_plugin.post("Test Title", "http://test.com/link")
    mock_post.assert_called_once()
