import pytest
from unittest.mock import patch, MagicMock
import sys

from src.main import main

@patch('src.main.load_config')
@patch('src.main.ArticleManager')
@patch('src.main.load_plugins')
def test_main_flow_new_articles_found(
    mock_load_plugins, mock_article_manager, mock_load_config
):
    """ 新しい記事が見つかった場合のメインフローをテスト """
    # --- Arrange ---
    # 設定のモック
    mock_load_config.return_value = {
        'blog': {'feed_url': 'http://test.com/feed'},
        'announcement_text': 'New Post:'
    }

    # ArticleManagerのモックインスタンスを作成
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance

    # ArticleManagerのメソッドの戻り値を設定
    mock_am_instance.get_latest_articles.return_value = [
        {'title': 'New Article', 'link': 'http://new.com'}
    ]
    mock_am_instance.load_saved_articles.return_value = []
    mock_am_instance.get_new_articles.return_value = [
        {'title': 'New Article', 'link': 'http://new.com'}
    ]
    mock_am_instance.create_post_text.return_value = "New Post: New Article http://new.com"

    # プラグインのモック
    mock_plugin = MagicMock()
    mock_load_plugins.return_value = {'test_plugin': mock_plugin}

    # --- Act ---
    # main関数を実行
    with patch.object(sys, 'argv', ['src/main.py']):
        main()

    # --- Assert ---
    # ArticleManagerのメソッドが呼ばれたか確認
    mock_am_instance.get_latest_articles.assert_called_once()
    mock_am_instance.load_saved_articles.assert_called_once()
    mock_am_instance.get_new_articles.assert_called_once()
    # create_post_textは各SNSに対して呼ばれるため、少なくとも1回は呼ばれることを確認
    assert mock_am_instance.create_post_text.called
    
    # プラグインが呼ばれたか確認
    mock_load_plugins.assert_called_once()
    # プラグインは最適化されたテキストで呼び出される
    assert mock_plugin.post.called

    # 記事が保存されたか確認
    mock_am_instance.save_articles.assert_called_once()

@patch('src.main.load_config')
@patch('src.main.ArticleManager')
@patch('builtins.print')
def test_main_flow_no_new_articles(mock_print, mock_article_manager, mock_load_config):
    """ 新しい記事がない場合のメインフローをテスト """
    # --- Arrange ---
    mock_load_config.return_value = {'blog': {'feed_url': 'http://test.com/feed'}}
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_new_articles.return_value = []

    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py']):
        main()

    # --- Assert ---
    mock_print.assert_any_call("新しい記事はありませんでした。")