import unittest
from unittest.mock import MagicMock, patch, mock_open
import json
import os
from src.plugins.tumblr import Tumblr

class TestTumblrPlugin(unittest.TestCase):

    def setUp(self):
        """テスト前の初期化"""
        self.client_id = "test_client_id"
        self.client_secret = "test_client_secret"
        self.access_token = "test_access_token"
        self.blog_name = "test_blog"
        self.test_config = {
            "tags": ["test", "auto-post"],
            "image_settings": {
                "max_width": 1024,
                "max_height": 1024
            }
        }

    def test_tumblr_init(self):
        """Tumblrプラグインの初期化テスト"""
        tumblr_plugin = Tumblr(
            self.client_id, 
            self.client_secret, 
            self.access_token, 
            self.blog_name, 
            self.test_config
        )

        self.assertEqual(tumblr_plugin.sns_type, "tumblr")
        self.assertEqual(tumblr_plugin.client_id, self.client_id)
        self.assertEqual(tumblr_plugin.client_secret, self.client_secret)
        self.assertEqual(tumblr_plugin.access_token, self.access_token)
        self.assertEqual(tumblr_plugin.blog_name, self.blog_name)
        self.assertEqual(tumblr_plugin.config, self.test_config)
        self.assertIn("Authorization", tumblr_plugin.headers)
        self.assertEqual(tumblr_plugin.headers["Authorization"], f"Bearer {self.access_token}")

    def test_supports_rich_content(self):
        """リッチコンテンツサポートのテスト"""
        tumblr_plugin = Tumblr(self.client_id, self.client_secret, self.access_token, self.blog_name)
        self.assertTrue(tumblr_plugin.supports_rich_content())

    @patch('src.plugins.tumblr.requests.post')
    def test_text_post_success(self, mock_post):
        """テキスト投稿成功のテスト"""
        # モックレスポンスの設定
        mock_response = MagicMock()
        mock_response.status_code = 201
        mock_response.json.return_value = {
            "response": {"id": "test_post_id"}
        }
        mock_post.return_value = mock_response

        tumblr_plugin = Tumblr(self.client_id, self.client_secret, self.access_token, self.blog_name)
        
        result = tumblr_plugin.post("テスト投稿です")
        
        # 投稿が成功することを確認
        self.assertTrue(result)
        
        # 正しいURLで投稿が実行されることを確認
        expected_url = f"https://api.tumblr.com/v2/blog/{self.blog_name}.tumblr.com/post"
        mock_post.assert_called_once()
        args, kwargs = mock_post.call_args
        self.assertEqual(args[0], expected_url)
        
        # JSON形式で投稿されることを確認
        self.assertIn('json', kwargs)
        post_data = kwargs['json']
        self.assertEqual(post_data['type'], 'text')
        self.assertEqual(post_data['body'], 'テスト投稿です')

    @patch('src.plugins.tumblr.requests.post')
    def test_text_post_with_tags(self, mock_post):
        """タグ付きテキスト投稿のテスト"""
        mock_response = MagicMock()
        mock_response.status_code = 201
        mock_response.json.return_value = {"response": {"id": "test_post_id"}}
        mock_post.return_value = mock_response

        tumblr_plugin = Tumblr(
            self.client_id, self.client_secret, self.access_token, 
            self.blog_name, self.test_config
        )
        
        result = tumblr_plugin.post("テスト投稿です")
        
        self.assertTrue(result)
        args, kwargs = mock_post.call_args
        post_data = kwargs['json']
        self.assertEqual(post_data['tags'], 'test,auto-post')

    @patch('src.plugins.tumblr.requests.post')
    def test_post_failure(self, mock_post):
        """投稿失敗のテスト"""
        mock_response = MagicMock()
        mock_response.status_code = 400
        mock_response.json.return_value = {
            "errors": [{"detail": "Invalid request"}]
        }
        mock_response.text = "Bad Request"
        mock_post.return_value = mock_response

        tumblr_plugin = Tumblr(self.client_id, self.client_secret, self.access_token, self.blog_name)
        
        result = tumblr_plugin.post("テスト投稿")
        
        self.assertFalse(result)

    def test_dry_run_mode(self):
        """ドライランモードのテスト"""
        tumblr_plugin = Tumblr(self.client_id, self.client_secret, self.access_token, self.blog_name)
        
        # ドライランでは実際の投稿は行われない
        result = tumblr_plugin.post("テスト投稿", dry_run=True, debug=True)
        
        self.assertTrue(result)

    @patch('src.plugins.tumblr.os.path.exists')
    @patch('src.plugins.tumblr.create_image_resizer')
    @patch('builtins.open', new_callable=mock_open, read_data=b'fake_image_data')
    @patch('src.plugins.tumblr.requests.post')
    def test_photo_post_success(self, mock_post, mock_file_open, mock_resizer, mock_exists):
        """画像付き投稿成功のテスト"""
        # モック設定
        mock_exists.return_value = True
        mock_resizer_instance = MagicMock()
        mock_resizer_instance.resize_image.return_value = "/path/to/resized/image.jpg"
        mock_resizer.return_value = mock_resizer_instance
        
        mock_response = MagicMock()
        mock_response.status_code = 201
        mock_response.json.return_value = {"response": {"id": "test_photo_post_id"}}
        mock_post.return_value = mock_response

        tumblr_plugin = Tumblr(self.client_id, self.client_secret, self.access_token, self.blog_name)
        
        result = tumblr_plugin.post("写真付き投稿", media_files=["/path/to/image.jpg"])
        
        self.assertTrue(result)
        
        # multipart/form-data で投稿されることを確認
        mock_post.assert_called_once()
        args, kwargs = mock_post.call_args
        self.assertIn('data', kwargs)
        self.assertIn('files', kwargs)
        
        # 投稿データの確認
        post_data = kwargs['data']
        self.assertEqual(post_data['type'], 'photo')
        self.assertEqual(post_data['caption'], '写真付き投稿')

    def test_prepare_post_data_text(self):
        """テキスト投稿データ準備のテスト"""
        tumblr_plugin = Tumblr(self.client_id, self.client_secret, self.access_token, self.blog_name)
        
        post_data = tumblr_plugin._prepare_post_data("テスト投稿", None, False)
        
        self.assertEqual(post_data['type'], 'text')
        self.assertEqual(post_data['body'], 'テスト投稿')

    @patch('src.plugins.tumblr.os.path.exists')
    def test_process_media_file_not_found(self, mock_exists):
        """存在しないメディアファイルの処理テスト"""
        mock_exists.return_value = False
        
        tumblr_plugin = Tumblr(self.client_id, self.client_secret, self.access_token, self.blog_name)
        
        result = tumblr_plugin._process_media_file("/nonexistent/file.jpg", True)
        
        self.assertIsNone(result)

    @patch('src.plugins.tumblr.requests.post')
    def test_handle_response_json_decode_error(self, mock_post):
        """JSONデコードエラー時の応答処理テスト"""
        mock_response = MagicMock()
        mock_response.status_code = 201
        mock_response.json.side_effect = json.JSONDecodeError("Invalid JSON", "", 0)
        mock_post.return_value = mock_response

        tumblr_plugin = Tumblr(self.client_id, self.client_secret, self.access_token, self.blog_name)
        
        result = tumblr_plugin.post("テスト投稿")
        
        # JSONデコードエラーでも成功として扱われる
        self.assertTrue(result)

    def test_post_exception_handling(self):
        """投稿時の例外処理テスト"""
        # 無効な設定で初期化（例外を発生させるため）
        tumblr_plugin = Tumblr("", "", "", "")
        
        # 例外が発生してもFalseが返されることを確認
        result = tumblr_plugin.post("テスト投稿")
        
        self.assertFalse(result)

if __name__ == '__main__':
    unittest.main()