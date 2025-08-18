import unittest
from unittest.mock import MagicMock, patch
import requests
from src.plugins.threads import Threads


class TestThreadsPlugin(unittest.TestCase):

    def setUp(self):
        """テスト用の設定"""
        self.app_id = "test_app_id"
        self.app_secret = "test_app_secret" 
        self.access_token = "test_access_token"
        self.test_user_id = "test_user_123"

    @patch('src.plugins.threads.requests.get')
    def test_threads_init_success(self, mock_get):
        """Threadsプラグインの正常な初期化テスト"""
        # ユーザーID取得のモック
        mock_response = MagicMock()
        mock_response.raise_for_status.return_value = None
        mock_response.json.return_value = {"id": self.test_user_id}
        mock_get.return_value = mock_response
        
        threads_plugin = Threads(self.app_id, self.app_secret, self.access_token)
        
        # 初期化の確認
        self.assertEqual(threads_plugin.sns_type, "threads")
        self.assertEqual(threads_plugin.app_id, self.app_id)
        self.assertEqual(threads_plugin.app_secret, self.app_secret)
        self.assertEqual(threads_plugin.access_token, self.access_token)
        self.assertEqual(threads_plugin.user_id, self.test_user_id)
        
        # APIコールの確認
        mock_get.assert_called_once_with(
            "https://graph.threads.net/v1.0/me",
            headers={"Authorization": f"Bearer {self.access_token}"},
            timeout=10
        )

    @patch('src.plugins.threads.requests.get')
    def test_threads_init_auth_failure(self, mock_get):
        """認証失敗時の初期化テスト"""
        # 認証失敗のモック
        mock_response = MagicMock()
        mock_response.raise_for_status.side_effect = requests.exceptions.HTTPError("401 Unauthorized")
        mock_get.return_value = mock_response
        
        with self.assertRaises(ValueError) as context:
            Threads(self.app_id, self.app_secret, self.access_token)
        
        self.assertIn("Threads APIへの認証に失敗しました", str(context.exception))

    @patch('src.plugins.threads.requests.get')
    def test_get_user_id_success(self, mock_get):
        """ユーザーID取得成功テスト"""
        # 正常レスポンスのモック
        mock_response = MagicMock()
        mock_response.raise_for_status.return_value = None
        mock_response.json.return_value = {"id": self.test_user_id, "username": "testuser"}
        mock_get.return_value = mock_response
        
        threads_plugin = Threads(self.app_id, self.app_secret, self.access_token)
        user_id = threads_plugin._get_user_id()
        
        self.assertEqual(user_id, self.test_user_id)

    @patch('src.plugins.threads.requests.get')
    def test_get_user_id_failure(self, mock_get):
        """ユーザーID取得失敗テスト"""
        # エラーレスポンスのモック
        mock_get.side_effect = requests.exceptions.RequestException("Network error")
        
        # 初期化時に失敗することを確認
        with self.assertRaises(ValueError):
            Threads(self.app_id, self.app_secret, self.access_token)

    @patch('src.plugins.threads.requests.get')
    @patch('src.plugins.threads.requests.post')
    def test_create_text_container_success(self, mock_post, mock_get):
        """テキストコンテナ作成成功テスト"""
        # 初期化用のモック
        mock_get_response = MagicMock()
        mock_get_response.raise_for_status.return_value = None
        mock_get_response.json.return_value = {"id": self.test_user_id}
        mock_get.return_value = mock_get_response
        
        # コンテナ作成のモック
        mock_post_response = MagicMock()
        mock_post_response.raise_for_status.return_value = None
        mock_post_response.json.return_value = {"id": "container_123"}
        mock_post.return_value = mock_post_response
        
        threads_plugin = Threads(self.app_id, self.app_secret, self.access_token)
        container_id = threads_plugin._create_text_container("テストポスト", debug=True)
        
        self.assertEqual(container_id, "container_123")
        
        # POSTリクエストの確認
        mock_post.assert_called_once()
        call_args = mock_post.call_args
        self.assertEqual(call_args[1]['data']['media_type'], 'TEXT')
        self.assertEqual(call_args[1]['data']['text'], 'テストポスト')

    @patch('src.plugins.threads.requests.get')
    @patch('src.plugins.threads.requests.post')
    def test_create_text_container_long_text(self, mock_post, mock_get):
        """長いテキストの文字数制限テスト"""
        # 初期化用のモック
        mock_get_response = MagicMock()
        mock_get_response.raise_for_status.return_value = None
        mock_get_response.json.return_value = {"id": self.test_user_id}
        mock_get.return_value = mock_get_response
        
        # コンテナ作成のモック
        mock_post_response = MagicMock()
        mock_post_response.raise_for_status.return_value = None
        mock_post_response.json.return_value = {"id": "container_123"}
        mock_post.return_value = mock_post_response
        
        threads_plugin = Threads(self.app_id, self.app_secret, self.access_token)
        
        # 500文字を超えるテキスト
        long_text = "あ" * 600
        container_id = threads_plugin._create_text_container(long_text, debug=True)
        
        self.assertEqual(container_id, "container_123")
        
        # 送信されたテキストが500文字以内に切り詰められているか確認
        call_args = mock_post.call_args
        sent_text = call_args[1]['data']['text']
        self.assertLessEqual(len(sent_text), 500)
        self.assertTrue(sent_text.endswith("..."))

    @patch('src.plugins.threads.requests.get')
    @patch('src.plugins.threads.requests.post')
    def test_publish_container_success(self, mock_post, mock_get):
        """コンテナ公開成功テスト"""
        # 初期化用のモック
        mock_get_response = MagicMock()
        mock_get_response.raise_for_status.return_value = None
        mock_get_response.json.return_value = {"id": self.test_user_id}
        mock_get.return_value = mock_get_response
        
        # コンテナ公開のモック
        mock_post_response = MagicMock()
        mock_post_response.raise_for_status.return_value = None
        mock_post_response.json.return_value = {"id": "thread_456"}
        mock_post.return_value = mock_post_response
        
        threads_plugin = Threads(self.app_id, self.app_secret, self.access_token)
        thread_id = threads_plugin._publish_container("container_123", debug=True)
        
        self.assertEqual(thread_id, "thread_456")
        
        # POSTリクエストの確認
        mock_post.assert_called_once()
        call_args = mock_post.call_args
        self.assertEqual(call_args[1]['data']['creation_id'], 'container_123')

    @patch('src.plugins.threads.requests.get')
    @patch('src.plugins.threads.requests.post')
    @patch('src.plugins.threads.time.sleep')
    def test_post_success(self, mock_sleep, mock_post, mock_get):
        """投稿成功テスト"""
        # 初期化用のモック
        mock_get_response = MagicMock()
        mock_get_response.raise_for_status.return_value = None
        mock_get_response.json.return_value = {"id": self.test_user_id}
        mock_get.return_value = mock_get_response
        
        # コンテナ作成と公開のモック
        mock_post.side_effect = [
            # コンテナ作成のレスポンス
            MagicMock(raise_for_status=lambda: None, json=lambda: {"id": "container_123"}),
            # コンテナ公開のレスポンス  
            MagicMock(raise_for_status=lambda: None, json=lambda: {"id": "thread_456"})
        ]
        
        threads_plugin = Threads(self.app_id, self.app_secret, self.access_token)
        
        # 投稿実行（例外が発生しないことを確認）
        try:
            threads_plugin.post("テスト投稿", debug=True)
        except Exception as e:
            self.fail(f"post method raised an exception: {e}")
        
        # sleep が呼ばれることを確認
        mock_sleep.assert_called_once_with(1)
        
        # 2回のPOSTリクエストが実行されることを確認
        self.assertEqual(mock_post.call_count, 2)

    @patch('src.plugins.threads.requests.get')
    def test_supports_rich_content(self, mock_get):
        """リッチコンテンツサポート確認テスト"""
        # 初期化用のモック
        mock_response = MagicMock()
        mock_response.raise_for_status.return_value = None
        mock_response.json.return_value = {"id": self.test_user_id}
        mock_get.return_value = mock_response
        
        threads_plugin = Threads(self.app_id, self.app_secret, self.access_token)
        
        # リッチコンテンツサポートは現在False
        self.assertFalse(threads_plugin.supports_rich_content())

    @patch('src.plugins.threads.requests.get')
    @patch('src.plugins.threads.requests.post')
    @patch('src.plugins.threads.time.sleep')
    def test_post_with_media_warning(self, mock_sleep, mock_post, mock_get):
        """メディア添付時の警告テスト"""
        # 初期化用のモック
        mock_get_response = MagicMock()
        mock_get_response.raise_for_status.return_value = None
        mock_get_response.json.return_value = {"id": self.test_user_id}
        mock_get.return_value = mock_get_response
        
        # コンテナ作成と公開のモック
        mock_post.side_effect = [
            MagicMock(raise_for_status=lambda: None, json=lambda: {"id": "container_123"}),
            MagicMock(raise_for_status=lambda: None, json=lambda: {"id": "thread_456"})
        ]
        
        threads_plugin = Threads(self.app_id, self.app_secret, self.access_token)
        
        # メディアファイル付きで投稿実行
        with patch('builtins.print') as mock_print:
            threads_plugin.post("テスト投稿", media_files=["test.jpg"], debug=True)
            
            # 警告メッセージが出力されることを確認
            warning_calls = [call for call in mock_print.call_args_list 
                           if "警告" in str(call) and "メディア添付" in str(call)]
            self.assertTrue(len(warning_calls) > 0)


if __name__ == '__main__':
    unittest.main()