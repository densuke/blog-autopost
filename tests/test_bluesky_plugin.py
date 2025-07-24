import unittest
from unittest.mock import MagicMock, patch
from src.plugins.bluesky import Bluesky
from atproto import Client, client_utils, models

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

    @patch('src.plugins.bluesky.Client')
    def test_hashtag_detection(self, MockClient):
        """ハッシュタグ検出機能のテスト"""
        mock_client_instance = MockClient.return_value
        bluesky_plugin = Bluesky("test_identifier", "test_password")
        
        # テストケース1: 単一ハッシュタグ
        text = "これは #テスト投稿 です"
        hashtags = bluesky_plugin._find_hashtags(text)
        self.assertEqual(len(hashtags), 1)
        self.assertEqual(hashtags[0][2], "テスト投稿")  # タグ名をチェック
        
        # バイト位置の正確性をチェック
        byte_start, byte_end, tag = hashtags[0]
        text_bytes = text.encode('utf-8')
        extracted = text_bytes[byte_start:byte_end].decode('utf-8')
        self.assertEqual(extracted, "#テスト投稿")  # バイト位置が正確にハッシュタグを指している
        
        # テストケース2: 複数ハッシュタグ
        text = "投稿テスト #blog #tech #test"
        hashtags = bluesky_plugin._find_hashtags(text)
        self.assertEqual(len(hashtags), 3)
        tags = [tag[2] for tag in hashtags]
        self.assertIn("blog", tags)
        self.assertIn("tech", tags)
        self.assertIn("test", tags)
        
        # テストケース3: 数字で始まるハッシュタグ（無効）
        text = "無効な #1test ハッシュタグ"
        hashtags = bluesky_plugin._find_hashtags(text)
        self.assertEqual(len(hashtags), 0)
        
        # テストケース4: 日本語ハッシュタグ
        text = "日本語 #ブログ #技術 投稿"
        hashtags = bluesky_plugin._find_hashtags(text)
        self.assertEqual(len(hashtags), 2)
        tags = [tag[2] for tag in hashtags]
        self.assertIn("ブログ", tags)
        self.assertIn("技術", tags)

    @patch('src.plugins.bluesky.Client')
    def test_facet_creation(self, MockClient):
        """facet生成機能のテスト"""
        mock_client_instance = MockClient.return_value
        bluesky_plugin = Bluesky("test_identifier", "test_password")
        
        # テストハッシュタグデータ（バイト位置、タグ名）
        hashtags = [(5, 10, "test"), (15, 22, "ブログ")]
        facets = bluesky_plugin._create_hashtag_facets(hashtags)
        
        self.assertEqual(len(facets), 2)
        
        # 最初のfacetをチェック
        facet1 = facets[0]
        self.assertIsInstance(facet1, models.AppBskyRichtextFacet.Main)
        self.assertEqual(facet1.index.byte_start, 5)
        self.assertEqual(facet1.index.byte_end, 10)
        self.assertEqual(facet1.features[0].tag, "test")
        
        # 2番目のfacetをチェック
        facet2 = facets[1]
        self.assertEqual(facet2.index.byte_start, 15)
        self.assertEqual(facet2.index.byte_end, 22)
        self.assertEqual(facet2.features[0].tag, "ブログ")

    @patch('src.plugins.bluesky.Client')
    def test_post_with_hashtags(self, MockClient):
        """ハッシュタグ付き投稿のテスト"""
        mock_client_instance = MockClient.return_value
        bluesky_plugin = Bluesky("test_identifier", "test_password")

        # Mock the send_post method
        mock_response = MagicMock()
        mock_response.uri = "at://did:plc:test/app.bsky.feed.post/test_uri"
        mock_client_instance.send_post.return_value = mock_response

        text_with_hashtags = "これは #テスト投稿 です #ブログ"
        bluesky_plugin.post(text_with_hashtags, debug=True)

        # send_postが呼ばれたことを確認
        mock_client_instance.send_post.assert_called_once()
        call_args, call_kwargs = mock_client_instance.send_post.call_args
        
        # facetsが含まれているかチェック
        self.assertIn('facets', call_kwargs)
        facets = call_kwargs['facets']
        self.assertIsInstance(facets, list)
        self.assertGreater(len(facets), 0)

if __name__ == '__main__':
    unittest.main()
