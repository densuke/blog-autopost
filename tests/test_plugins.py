from unittest.mock import MagicMock, patch

from src.plugins.x import X

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
