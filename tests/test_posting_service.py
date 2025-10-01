#!/usr/bin/env python
# -*- coding: utf-8 -*-

import pytest
from unittest.mock import MagicMock, patch

@pytest.fixture
def mock_dependencies():
    """PostingServiceの依存関係のモックを作成する"""
    return {
        'config_manager': MagicMock(),
        'image_resizer': MagicMock(),
        'text_optimizer': MagicMock()
    }

@patch('src.web.posting_service.plugin_loader.load_plugins')
@patch('src.web.posting_service.validate_media_for_posting')
def test_post_now_success(mock_validate_media_for_posting, mock_load_plugins, mock_dependencies):
    """投稿処理が成功するケースをテストする（リッチコンテンツ非対応プラグイン）"""
    from src.web.posting_service import PostingService
    
    # モックの設定
    mock_plugin = MagicMock()
    mock_plugin.sns_type = 'x'
    mock_plugin.supports_rich_content.return_value = False # リッチコンテンツ非対応
    mock_load_plugins.return_value = {'x-main': mock_plugin}
    mock_dependencies['text_optimizer'].optimize_text.return_value = "Optimized text with URL"
    mock_validate_media_for_posting.return_value = {'x': MagicMock(is_valid=True, errors=[])}

    service = PostingService(**mock_dependencies)
    
    post_data = {
        'text': 'Original text',
        'url': 'http://example.com',
        'media_files': [],
        'sns_targets': ['x-main']
    }

    result = service.post_now(post_data)

    # 各サービスが正しく呼び出されたか検証
    mock_load_plugins.assert_called_once_with(mock_dependencies['config_manager'], sns_names=['x-main'])
    mock_validate_media_for_posting.assert_called_once()
    mock_dependencies['text_optimizer'].optimize_text.assert_called_once_with("Original text http://example.com", 'http://example.com', 'x')
    mock_plugin.post.assert_called_once_with("Optimized text with URL", [], article_data=None, debug=False)

    # 結果を検証
    assert result['x-main']['success'] is True

@patch('src.web.posting_service.plugin_loader.load_plugins')
@patch('src.web.posting_service.validate_media_for_posting')
def test_post_now_with_rich_content_plugin(mock_validate_media_for_posting, mock_load_plugins, mock_dependencies):
    """投稿処理が成功するケースをテストする（リッチコンテンツ対応プラグイン）"""
    from src.web.posting_service import PostingService
    
    # モックの設定
    mock_plugin = MagicMock()
    mock_plugin.sns_type = 'bluesky'
    mock_plugin.supports_rich_content.return_value = True # リッチコンテンツ対応
    mock_load_plugins.return_value = {'bluesky-main': mock_plugin}
    mock_dependencies['text_optimizer'].optimize_text.return_value = "Optimized text without URL"
    mock_validate_media_for_posting.return_value = {'bluesky': MagicMock(is_valid=True, errors=[])}

    service = PostingService(**mock_dependencies)
    
    post_data = {
        'text': 'Original text',
        'url': 'http://example.com',
        'media_files': [],
        'sns_targets': ['bluesky-main']
    }

    result = service.post_now(post_data)

    # 各サービスが正しく呼び出されたか検証
    mock_load_plugins.assert_called_once_with(mock_dependencies['config_manager'], sns_names=['bluesky-main'])
    mock_validate_media_for_posting.assert_called_once()
    mock_dependencies['text_optimizer'].optimize_text.assert_called_once_with("Original text", 'http://example.com', 'bluesky')
    mock_plugin.post.assert_called_once_with(
        "Optimized text without URL", 
        [], 
        article_data={'title': 'Original text', 'link': 'http://example.com', 'description': 'Original text'}, 
        debug=False
    )

    # 結果を検証
    assert result['bluesky-main']['success'] is True
