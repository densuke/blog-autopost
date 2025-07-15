import unittest
from unittest.mock import MagicMock, patch
from src.plugins.bluesky import Bluesky
from atproto import Client, client_utils

class TestBlueskyPlugin(unittest.TestCase):

    @patch('src.plugins.bluesky.Client')
    def test_bluesky_init(self, MockClient):
        mock_client_instance = MockClient.return_value
        bluesky_plugin = Bluesky("test_identifier", "test_password")

        MockClient.assert_called_once()
        mock_client_instance.login.assert_called_once_with("test_identifier", "test_password")
        self.assertEqual(bluesky_plugin.client, mock_client_instance)

    @patch('src.plugins.bluesky.Client')
    def test_bluesky_post(self, MockClient):
        mock_client_instance = MockClient.return_value
        bluesky_plugin = Bluesky("test_identifier", "test_password")

        title = "Test Title"
        link = "http://test.com/link"

        # Mock the send_post method to return a mock response
        mock_response = MagicMock()
        mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test_uri"
        mock_client_instance.send_post.return_value = mock_response

        bluesky_plugin.post(title, link)

        # Verify that send_post was called with the correct text
        expected_text_builder = client_utils.TextBuilder().text(f"{title} ").link(link, link)
        mock_client_instance.send_post.assert_called_once()
        call_args, call_kwargs = mock_client_instance.send_post.call_args
        self.assertIn('text', call_kwargs)
        self.assertIsInstance(call_kwargs['text'], client_utils.TextBuilder)

if __name__ == '__main__':
    unittest.main()
