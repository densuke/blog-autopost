#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""posting_service.py の高度なテスト

バックグラウンド処理やメディア処理の複雑なシナリオをテストする。
"""

import os
import tempfile
import shutil
from unittest.mock import patch, MagicMock, call

import pytest

from src.web.posting_service import PostingService
from src.config_manager import ConfigManager
from src.image_resizer import ImageResizer
from src.text_optimizer import TextOptimizer


class TestPostingServiceAdvanced:
    """PostingService クラスの高度なテスト"""

    @pytest.fixture(autouse=True)
    def setup_method(self):
        """各テスト前に実行"""
        # モックの初期化
        self.mock_config = MagicMock(spec=ConfigManager)
        self.mock_resizer = MagicMock(spec=ImageResizer)
        self.mock_optimizer = MagicMock(spec=TextOptimizer)

        self.service = PostingService(
            config_manager=self.mock_config,
            image_resizer=self.mock_resizer,
            text_optimizer=self.mock_optimizer
        )

        # テンポラリディレクトリ
        self.temp_dir = tempfile.mkdtemp()

    def teardown_method(self):
        """各テスト後に実行"""
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)

    def test_post_now_empty_sns_targets(self):
        """SNS対象なしテスト"""
        post_data = {
            'text': 'Test post',
            'sns_targets': [],
            'media_files': []
        }

        result = self.service.post_now(post_data)

        assert 'error' in result
        assert result['error'] == 'No valid SNS targets found or loaded.'

    @patch('src.web.posting_service.plugin_loader')
    def test_post_now_with_media_validation_failure(self, mock_loader):
        """メディア検証失敗テスト"""
        # プラグインのモック
        mock_plugin = MagicMock()
        mock_plugin.sns_type = 'x'
        mock_loader.load_plugins.return_value = {'x_plugin': mock_plugin}

        # メディア検証のモック（失敗）
        with patch('src.web.posting_service.validate_media_for_posting') as mock_validate:
            mock_validation_result = MagicMock()
            mock_validation_result.is_valid = False
            mock_validation_result.errors = ['Invalid image format', 'Size too large']
            mock_validate.return_value = {'x': mock_validation_result}

            post_data = {
                'text': 'Test post',
                'sns_targets': ['x'],
                'media_files': ['/tmp/invalid.jpg']
            }

            result = self.service.post_now(post_data)

            # メディア検証エラーが報告されたことを確認
            assert 'x_plugin' in result
            assert not result['x_plugin']['success']
            assert 'Media validation failed' in result['x_plugin']['message']

    @patch('src.web.posting_service.plugin_loader')
    def test_post_now_with_image_resize(self, mock_loader):
        """画像リサイズ処理テスト"""
        # テンポラリイメージを作成
        temp_image = os.path.join(self.temp_dir, 'test.jpg')
        with open(temp_image, 'w') as f:
            f.write('fake image data')

        # プラグインのモック
        mock_plugin = MagicMock()
        mock_plugin.sns_type = 'x'
        mock_plugin.supports_rich_content = MagicMock(return_value=False)
        mock_loader.load_plugins.return_value = {'x_plugin': mock_plugin}

        # リサイザーのモック
        resized_image = os.path.join(self.temp_dir, 'resized.jpg')
        with open(resized_image, 'w') as f:
            f.write('resized image data')

        self.mock_resizer.resize_image_file.return_value = resized_image
        self.mock_optimizer.optimize_text.return_value = 'Optimized text'

        with patch('src.web.posting_service.validate_media_for_posting') as mock_validate:
            mock_validation_result = MagicMock()
            mock_validation_result.is_valid = True
            mock_validate.return_value = {'x': mock_validation_result}

            post_data = {
                'text': 'Test post',
                'url': '',
                'sns_targets': ['x'],
                'media_files': [temp_image]
            }

            result = self.service.post_now(post_data)

            # リサイザーが呼び出されたことを確認
            self.mock_resizer.resize_image_file.assert_called_with(temp_image, 'x')

            # プラグインのpostメソッドが呼び出されたことを確認
            mock_plugin.post.assert_called_once()
            call_args = mock_plugin.post.call_args
            # リサイズ済みメディアが渡されたことを確認
            assert resized_image in call_args[0][1]

    @patch('src.web.posting_service.plugin_loader')
    def test_post_now_with_url_rich_content(self, mock_loader):
        """URLとリッチコンテンツのテスト"""
        # プラグインのモック（リッチコンテンツ対応）
        mock_plugin = MagicMock()
        mock_plugin.sns_type = 'bluesky'
        mock_plugin.supports_rich_content = MagicMock(return_value=True)
        mock_loader.load_plugins.return_value = {'bluesky_plugin': mock_plugin}

        self.mock_optimizer.optimize_text.return_value = 'Optimized text'

        with patch('src.web.posting_service.validate_media_for_posting') as mock_validate:
            mock_validation_result = MagicMock()
            mock_validation_result.is_valid = True
            mock_validate.return_value = {'bluesky': mock_validation_result}

            post_data = {
                'text': 'Check this out',
                'url': 'https://example.com',
                'sns_targets': ['bluesky'],
                'media_files': []
            }

            result = self.service.post_now(post_data)

            # postメソッドが呼び出されたことを確認
            mock_plugin.post.assert_called_once()
            call_args = mock_plugin.post.call_args

            # article_dataが渡されたことを確認
            assert call_args[1]['article_data'] is not None
            article_data = call_args[1]['article_data']
            assert article_data['link'] == 'https://example.com'

    @patch('src.web.posting_service.plugin_loader')
    def test_post_now_with_url_no_rich_content(self, mock_loader):
        """URLとリッチコンテンツ非対応のテスト"""
        # プラグインのモック（リッチコンテンツ非対応）
        mock_plugin = MagicMock()
        mock_plugin.sns_type = 'x'
        mock_plugin.supports_rich_content = MagicMock(return_value=False)
        mock_loader.load_plugins.return_value = {'x_plugin': mock_plugin}

        self.mock_optimizer.optimize_text.return_value = 'Check this out https://example.com'

        with patch('src.web.posting_service.validate_media_for_posting') as mock_validate:
            mock_validation_result = MagicMock()
            mock_validation_result.is_valid = True
            mock_validate.return_value = {'x': mock_validation_result}

            post_data = {
                'text': 'Check this out',
                'url': 'https://example.com',
                'sns_targets': ['x'],
                'media_files': []
            }

            result = self.service.post_now(post_data)

            # postメソッドが呼び出されたことを確認
            mock_plugin.post.assert_called_once()
            call_args = mock_plugin.post.call_args

            # テキストにURLが含まれたことを確認
            optimized_text = call_args[0][0]
            assert 'https://example.com' in optimized_text

    @patch('src.web.posting_service.plugin_loader')
    def test_post_now_exception_handling(self, mock_loader):
        """例外ハンドリングテスト"""
        # プラグインのモック（例外を発生）
        mock_plugin = MagicMock()
        mock_plugin.sns_type = 'mastodon'
        mock_plugin.post.side_effect = Exception('Network error')
        mock_loader.load_plugins.return_value = {'mastodon_plugin': mock_plugin}

        self.mock_optimizer.optimize_text.return_value = 'Optimized text'

        with patch('src.web.posting_service.validate_media_for_posting') as mock_validate:
            mock_validation_result = MagicMock()
            mock_validation_result.is_valid = True
            mock_validate.return_value = {'mastodon': mock_validation_result}

            post_data = {
                'text': 'Test post',
                'url': '',
                'sns_targets': ['mastodon'],
                'media_files': []
            }

            result = self.service.post_now(post_data)

            # エラーが記録されたことを確認
            assert 'mastodon_plugin' in result
            assert not result['mastodon_plugin']['success']
            assert 'Network error' in result['mastodon_plugin']['message']

    def test_post_now_and_cleanup_success(self):
        """投稿後のクリーンアップ成功テスト"""
        # テンポラリメディアディレクトリを作成
        job_media_dir = os.path.join(self.temp_dir, 'job_media')
        os.makedirs(job_media_dir, exist_ok=True)

        # ダミーファイルを作成
        media_file = os.path.join(job_media_dir, 'test.jpg')
        with open(media_file, 'w') as f:
            f.write('test')

        assert os.path.exists(job_media_dir)

        with patch.object(self.service, 'post_now') as mock_post:
            mock_post.return_value = {'x': {'success': True}}

            post_data = {
                'text': 'Test',
                'sns_targets': ['x'],
                'media_files': [],
                'job_media_dir': job_media_dir
            }

            result = self.service.post_now_and_cleanup(post_data)

            # post_nowが呼び出されたことを確認
            mock_post.assert_called_once_with(post_data, False)

            # メディアディレクトリが削除されたことを確認
            assert not os.path.exists(job_media_dir)

    def test_post_now_and_cleanup_missing_directory(self):
        """存在しないメディアディレクトリのクリーンアップテスト"""
        post_data = {
            'text': 'Test',
            'sns_targets': ['x'],
            'media_files': [],
            'job_media_dir': '/nonexistent/directory'
        }

        with patch.object(self.service, 'post_now') as mock_post:
            mock_post.return_value = {'x': {'success': True}}

            # エラーが発生しないことを確認
            result = self.service.post_now_and_cleanup(post_data)

            assert result == {'x': {'success': True}}

    @patch('src.web.posting_service.plugin_loader')
    def test_post_now_multiple_sns(self, mock_loader):
        """複数SNS投稿テスト"""
        # 複数プラグインのモック
        plugins = {}
        for sns in ['x', 'bluesky', 'mastodon']:
            mock_plugin = MagicMock()
            mock_plugin.sns_type = sns
            mock_plugin.supports_rich_content = MagicMock(return_value=False)
            plugins[f'{sns}_plugin'] = mock_plugin

        mock_loader.load_plugins.return_value = plugins

        self.mock_optimizer.optimize_text.return_value = 'Optimized text'

        with patch('src.web.posting_service.validate_media_for_posting') as mock_validate:
            validation_results = {}
            for sns in ['x', 'bluesky', 'mastodon']:
                mock_validation_result = MagicMock()
                mock_validation_result.is_valid = True
                validation_results[sns] = mock_validation_result

            mock_validate.return_value = validation_results

            post_data = {
                'text': 'Test post',
                'url': '',
                'sns_targets': ['x', 'bluesky', 'mastodon'],
                'media_files': []
            }

            result = self.service.post_now(post_data)

            # すべてのプラグインが実行されたことを確認
            assert 'x_plugin' in result
            assert 'bluesky_plugin' in result
            assert 'mastodon_plugin' in result
