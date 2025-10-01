#!/usr/bin/env python
# -*- coding: utf-8 -*-

import pytest
from unittest.mock import MagicMock, patch

# PostingServiceがまだ存在しないため、このインポートは失敗する
# from src.web.posting_service import PostingService

@pytest.fixture
def mock_dependencies():
    """PostingServiceの依存関係のモックを作成する"""
    return {
        'config_manager': MagicMock(),
        'media_validator': MagicMock(),
        'image_resizer': MagicMock(),
        'text_optimizer': MagicMock()
    }

@patch('src.web.posting_service.plugin_loader.load_plugins')
def test_post_now_success(mock_load_plugins, mock_dependencies):
    """投稿処理が成功するケースをテストする"""
    from src.web.posting_service import PostingService
    
    # モックの設定
    mock_plugin = MagicMock()
    mock_plugin.sns_type = 'x' # sns_type属性を追加
    mock_load_plugins.return_value = {'x-main': mock_plugin}
    mock_dependencies['text_optimizer'].optimize_text.return_value = "Optimized text"

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
    mock_dependencies['media_validator'].validate_media_for_posting.assert_called_once()
    mock_dependencies['text_optimizer'].optimize_text.assert_called_once()
    mock_plugin.post.assert_called_once_with("Optimized text", [], article_data=None, debug=False)

    # 結果を検証
    assert result['x-main']['success'] is True
