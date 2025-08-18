
import pytest
from src.article_manager import ArticleManager
from src.config_manager import ConfigManager

def test_announcement_text_added():
    """
    announcement_textが設定されている場合に、投稿文の先頭に付与されることを確認する
    """
    config = {
        'announcement_text': 'ブログ更新:',
        'blog': {'feed_url': 'dummy_url'},
        'sns': {}
    }
    article_manager = ArticleManager(ConfigManager(config))
    post_text = article_manager.create_post_text("Test Title", "http://example.com", "x")
    assert post_text == "ブログ更新: Test Title http://example.com"

def test_announcement_text_empty():
    """
    announcement_textが空文字列の場合に、何も付与されないことを確認する
    """
    config = {
        'announcement_text': '',
        'blog': {'feed_url': 'dummy_url'},
        'sns': {}
    }
    article_manager = ArticleManager(ConfigManager(config))
    post_text = article_manager.create_post_text("Test Title", "http://example.com", "x")
    assert post_text == "Test Title http://example.com"

def test_announcement_text_missing():
    """
    announcement_textキーが存在しない場合に、何も付与されないことを確認する
    """
    config = {
        'blog': {'feed_url': 'dummy_url'},
        'sns': {}
    }
    article_manager = ArticleManager(ConfigManager(config))
    post_text = article_manager.create_post_text("Test Title", "http://example.com", "x")
    assert post_text == "Test Title http://example.com"
