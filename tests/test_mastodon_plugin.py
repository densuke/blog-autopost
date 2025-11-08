#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
Mastodonプラグインのテスト
"""

import tempfile
from unittest.mock import Mock, mock_open, patch

import requests

from src.plugins.mastodon import Mastodon


class TestMastodonPlugin:
    """Mastodonプラグインのテスト"""

    def test_init(self):
        """初期化が正常に行われること"""
        plugin = Mastodon("https://mastodon.example", "test_token")
        assert plugin.sns_type == "mastodon"
        assert plugin.base_url == "https://mastodon.example"
        assert plugin.access_token == "test_token"

    def test_init_strips_trailing_slash(self):
        """instance_urlの末尾のスラッシュが削除されること"""
        plugin = Mastodon("https://mastodon.example/", "test_token")
        assert plugin.base_url == "https://mastodon.example"

    @patch('requests.post')
    def test_post_text_only_success(self, mock_post):
        """テキストのみの投稿が成功すること"""
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "id": "mastodon_test_id",
            "url": "https://mastodon.example/@user/123"
        }
        mock_post.return_value = mock_response

        plugin = Mastodon("https://mastodon.example", "test_token")
        plugin.post("Test Title")

        # APIが正しく呼び出されたかを確認
        mock_post.assert_called_once_with(
            "https://mastodon.example/api/v1/statuses",
            headers={
                "Authorization": "Bearer test_token",
                "User-Agent": "blog-autopost/1.0",
                "Content-Type": "application/json"
            },
            json={"status": "Test Title"}
        )

    @patch('requests.post')
    def test_post_with_status_201(self, mock_post):
        """ステータスコード201でも成功すること"""
        mock_response = Mock()
        mock_response.status_code = 201
        mock_response.json.return_value = {
            "id": "test_id",
            "url": "https://mastodon.example/@user/123"
        }
        mock_post.return_value = mock_response

        plugin = Mastodon("https://mastodon.example", "test_token")
        plugin.post("Test content")

        mock_post.assert_called_once()

    @patch('requests.post')
    def test_post_with_media_success(self, mock_post):
        """メディア付き投稿が成功すること"""
        # メディアアップロードのモック
        media_response = Mock()
        media_response.status_code = 200
        media_response.json.return_value = {"id": "media_123"}

        # 投稿のモック
        post_response = Mock()
        post_response.status_code = 200
        post_response.json.return_value = {
            "id": "post_456",
            "url": "https://mastodon.example/@user/456"
        }

        mock_post.side_effect = [media_response, post_response]

        with tempfile.NamedTemporaryFile(suffix=".jpg") as tmp_file:
            tmp_file.write(b"fake image data")
            tmp_file.flush()

            with patch('builtins.open', mock_open(read_data=b"fake image data")):
                plugin = Mastodon("https://mastodon.example", "test_token")
                plugin.post("Test with media", media_files=[tmp_file.name])

        # メディアアップロードと投稿の2回呼ばれること
        assert mock_post.call_count == 2

        # 投稿時にmedia_idsが含まれていること
        post_call = mock_post.call_args_list[1]
        assert post_call[1]['json']['media_ids'] == ["media_123"]

    @patch('requests.post')
    def test_post_with_multiple_media(self, mock_post):
        """複数のメディアファイルをアップロードできること"""
        # メディアアップロードのモック
        media_response1 = Mock()
        media_response1.status_code = 200
        media_response1.json.return_value = {"id": "media_1"}

        media_response2 = Mock()
        media_response2.status_code = 200
        media_response2.json.return_value = {"id": "media_2"}

        # 投稿のモック
        post_response = Mock()
        post_response.status_code = 200
        post_response.json.return_value = {"id": "post_123", "url": "https://example.com"}

        mock_post.side_effect = [media_response1, media_response2, post_response]

        with tempfile.NamedTemporaryFile(suffix=".jpg") as tmp1:
            with tempfile.NamedTemporaryFile(suffix=".png") as tmp2:
                tmp1.write(b"image1")
                tmp2.write(b"image2")
                tmp1.flush()
                tmp2.flush()

                with patch('builtins.open', mock_open(read_data=b"fake data")):
                    plugin = Mastodon("https://mastodon.example", "test_token")
                    plugin.post("Multiple media", media_files=[tmp1.name, tmp2.name])

        # 3回呼ばれること（メディア2回 + 投稿1回）
        assert mock_post.call_count == 3

        # 投稿時に両方のmedia_idsが含まれていること
        post_call = mock_post.call_args_list[2]
        assert post_call[1]['json']['media_ids'] == ["media_1", "media_2"]

    @patch('requests.post')
    def test_post_error_status_code(self, mock_post):
        """投稿エラー時にエラーメッセージが表示されること"""
        mock_response = Mock()
        mock_response.status_code = 400
        mock_response.text = "Bad Request"
        mock_response.json.return_value = {"error": "Invalid status"}
        mock_post.return_value = mock_response

        plugin = Mastodon("https://mastodon.example", "test_token")
        plugin.post("Test Title")

        mock_post.assert_called_once()

    @patch('requests.post')
    def test_post_error_json_parse_error(self, mock_post):
        """エラーレスポンスのJSON解析に失敗した場合の処理"""
        mock_response = Mock()
        mock_response.status_code = 500
        mock_response.text = "Internal Server Error"
        mock_response.json.side_effect = ValueError("Invalid JSON")
        mock_post.return_value = mock_response

        plugin = Mastodon("https://mastodon.example", "test_token")
        plugin.post("Test")

        mock_post.assert_called_once()

    @patch('requests.post')
    def test_post_request_exception(self, mock_post):
        """リクエスト例外が発生した場合の処理"""
        mock_post.side_effect = requests.exceptions.RequestException("Connection error")

        plugin = Mastodon("https://mastodon.example", "test_token")
        plugin.post("Test")

        mock_post.assert_called_once()

    @patch('requests.post')
    def test_post_unexpected_exception(self, mock_post):
        """予期しない例外が発生した場合の処理"""
        mock_post.side_effect = RuntimeError("Unexpected error")

        plugin = Mastodon("https://mastodon.example", "test_token")
        plugin.post("Test")

        mock_post.assert_called_once()

    @patch('requests.post')
    def test_upload_media_file_not_found(self, mock_post):
        """存在しないメディアファイルの場合、Noneを返すこと"""
        plugin = Mastodon("https://mastodon.example", "test_token")
        result = plugin._upload_media("/nonexistent/file.jpg", {})
        assert result is None
        mock_post.assert_not_called()

    @patch('requests.post')
    def test_upload_media_success(self, mock_post):
        """メディアアップロードが成功すること"""
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {"id": "media_123"}
        mock_post.return_value = mock_response

        with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_file:
            tmp_file.write(b"test image")
            tmp_file_path = tmp_file.name

        try:
            with patch('builtins.open', mock_open(read_data=b"test image")):
                plugin = Mastodon("https://mastodon.example", "test_token")
                headers = {"Authorization": "Bearer test_token"}
                result = plugin._upload_media(tmp_file_path, headers)

            assert result == "media_123"
            mock_post.assert_called_once()
        finally:
            import os
            if os.path.exists(tmp_file_path):
                os.remove(tmp_file_path)

    @patch('requests.post')
    def test_upload_media_status_201(self, mock_post):
        """ステータスコード201でも成功すること"""
        mock_response = Mock()
        mock_response.status_code = 201
        mock_response.json.return_value = {"id": "media_456"}
        mock_post.return_value = mock_response

        with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_file:
            tmp_file.write(b"test")
            tmp_file_path = tmp_file.name

        try:
            with patch('builtins.open', mock_open(read_data=b"test")):
                plugin = Mastodon("https://mastodon.example", "test_token")
                result = plugin._upload_media(tmp_file_path, {})

            assert result == "media_456"
        finally:
            import os
            if os.path.exists(tmp_file_path):
                os.remove(tmp_file_path)

    @patch('requests.post')
    @patch('time.sleep')
    def test_upload_media_status_202_async_processing(self, mock_sleep, mock_post):
        """ステータスコード202（非同期処理）でも成功すること"""
        mock_response = Mock()
        mock_response.status_code = 202
        mock_response.json.return_value = {"id": "media_789"}
        mock_post.return_value = mock_response

        with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_file:
            tmp_file.write(b"test")
            tmp_file_path = tmp_file.name

        try:
            with patch('builtins.open', mock_open(read_data=b"test")):
                plugin = Mastodon("https://mastodon.example", "test_token")
                result = plugin._upload_media(tmp_file_path, {})

            assert result == "media_789"
            # 非同期処理の待機が行われたことを確認
            mock_sleep.assert_called_once_with(1)
        finally:
            import os
            if os.path.exists(tmp_file_path):
                os.remove(tmp_file_path)

    @patch('requests.post')
    def test_upload_media_error_status_code(self, mock_post):
        """メディアアップロードエラー時にNoneを返すこと"""
        mock_response = Mock()
        mock_response.status_code = 400
        mock_response.text = "Bad Request"
        mock_response.json.return_value = {"error": "Invalid file"}
        mock_post.return_value = mock_response

        with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_file:
            tmp_file.write(b"test")
            tmp_file_path = tmp_file.name

        try:
            with patch('builtins.open', mock_open(read_data=b"test")):
                plugin = Mastodon("https://mastodon.example", "test_token")
                result = plugin._upload_media(tmp_file_path, {})

            assert result is None
        finally:
            import os
            if os.path.exists(tmp_file_path):
                os.remove(tmp_file_path)

    @patch('requests.post')
    def test_upload_media_error_json_parse_error(self, mock_post):
        """メディアアップロードエラーレスポンスのJSON解析失敗時の処理"""
        mock_response = Mock()
        mock_response.status_code = 500
        mock_response.text = "Internal Server Error"
        mock_response.json.side_effect = ValueError("Invalid JSON")
        mock_post.return_value = mock_response

        with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_file:
            tmp_file.write(b"test")
            tmp_file_path = tmp_file.name

        try:
            with patch('builtins.open', mock_open(read_data=b"test")):
                plugin = Mastodon("https://mastodon.example", "test_token")
                result = plugin._upload_media(tmp_file_path, {})

            assert result is None
        finally:
            import os
            if os.path.exists(tmp_file_path):
                os.remove(tmp_file_path)

    @patch('requests.post')
    def test_upload_media_request_exception(self, mock_post):
        """メディアアップロード時のリクエスト例外処理"""
        mock_post.side_effect = requests.exceptions.RequestException("Connection error")

        with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_file:
            tmp_file.write(b"test")
            tmp_file_path = tmp_file.name

        try:
            with patch('builtins.open', mock_open(read_data=b"test")):
                plugin = Mastodon("https://mastodon.example", "test_token")
                result = plugin._upload_media(tmp_file_path, {})

            assert result is None
        finally:
            import os
            if os.path.exists(tmp_file_path):
                os.remove(tmp_file_path)

    @patch('requests.post')
    def test_upload_media_unexpected_exception(self, mock_post):
        """メディアアップロード時の予期しない例外処理"""
        mock_post.side_effect = RuntimeError("Unexpected error")

        with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_file:
            tmp_file.write(b"test")
            tmp_file_path = tmp_file.name

        try:
            with patch('builtins.open', mock_open(read_data=b"test")):
                plugin = Mastodon("https://mastodon.example", "test_token")
                result = plugin._upload_media(tmp_file_path, {})

            assert result is None
        finally:
            import os
            if os.path.exists(tmp_file_path):
                os.remove(tmp_file_path)

    @patch('requests.post')
    def test_post_with_media_partial_upload_failure(self, mock_post):
        """一部のメディアアップロードが失敗しても投稿は続行されること"""
        # 1つ目は成功、2つ目は失敗
        media_response1 = Mock()
        media_response1.status_code = 200
        media_response1.json.return_value = {"id": "media_1"}

        media_response2 = Mock()
        media_response2.status_code = 400
        media_response2.json.return_value = {"error": "Invalid"}

        post_response = Mock()
        post_response.status_code = 200
        post_response.json.return_value = {"id": "post_123", "url": "https://example.com"}

        mock_post.side_effect = [media_response1, media_response2, post_response]

        with tempfile.NamedTemporaryFile(suffix=".jpg") as tmp1:
            with tempfile.NamedTemporaryFile(suffix=".png") as tmp2:
                tmp1.write(b"image1")
                tmp2.write(b"image2")
                tmp1.flush()
                tmp2.flush()

                with patch('builtins.open', mock_open(read_data=b"fake data")):
                    plugin = Mastodon("https://mastodon.example", "test_token")
                    plugin.post("Test", media_files=[tmp1.name, tmp2.name])

        # 投稿は成功していること
        assert mock_post.call_count == 3
        # 成功したメディアのみが含まれること
        post_call = mock_post.call_args_list[2]
        assert post_call[1]['json']['media_ids'] == ["media_1"]
