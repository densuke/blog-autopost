import pytest
from unittest.mock import MagicMock, patch
import json
import os
from datetime import datetime, timezone

from src.article_manager import ArticleManager, DATA_FILE

# DATA_FILEをテスト用に一時的に変更
@pytest.fixture(autouse=True)
def mock_data_file(tmp_path):
    original_data_file = DATA_FILE
    ArticleManager.DATA_FILE = tmp_path / "test_articles.json"
    yield
    ArticleManager.DATA_FILE = original_data_file

@pytest.fixture
def mock_config_manager():
    mock_cm = MagicMock()
    mock_cm.get_all_feed_configs.return_value = [
        {'name': 'feed1', 'feed_url': 'http://example.com/feed1'},
        {'name': 'feed2', 'feed_url': 'http://example.com/feed2'}
    ]
    mock_cm.get_feed_config_by_name.side_effect = lambda name: {
        'feed1': {'name': 'feed1', 'feed_url': 'http://example.com/feed1'},
        'feed2': {'name': 'feed2', 'feed_url': 'http://example.com/feed2'}
    }.get(name)
    return mock_cm

@pytest.fixture
def mock_feedparser():
    with patch('feedparser.parse') as mock_parse:
        # feed1のモック
        mock_feed1_entry1 = MagicMock()
        mock_feed1_entry1.title = "Article 1 from Feed 1"
        mock_feed1_entry1.link = "http://example.com/feed1/article1"
        mock_feed1_entry1.published = "Mon, 01 Jan 2024 10:00:00 GMT"
        mock_feed1_entry1.published_parsed = datetime(2024, 1, 1, 10, 0, 0, tzinfo=timezone.utc).timetuple()

        mock_feed1_entry2 = MagicMock()
        mock_feed1_entry2.title = "Article 2 from Feed 1"
        mock_feed1_entry2.link = "http://example.com/feed1/article2"
        mock_feed1_entry2.published = "Mon, 01 Jan 2024 11:00:00 GMT"
        mock_feed1_entry2.published_parsed = datetime(2024, 1, 1, 11, 0, 0, tzinfo=timezone.utc).timetuple()

        mock_feed1 = MagicMock()
        mock_feed1.bozo = 0
        mock_feed1.entries = [mock_feed1_entry1, mock_feed1_entry2]

        # feed2のモック
        mock_feed2_entry1 = MagicMock()
        mock_feed2_entry1.title = "Article 1 from Feed 2"
        mock_feed2_entry1.link = "http://example.com/feed2/article1"
        mock_feed2_entry1.published = "Mon, 01 Jan 2024 12:00:00 GMT"
        mock_feed2_entry1.published_parsed = datetime(2024, 1, 1, 12, 0, 0, tzinfo=timezone.utc).timetuple()

        mock_feed2 = MagicMock()
        mock_feed2.bozo = 0
        mock_feed2.entries = [mock_feed2_entry1]

        mock_parse.side_effect = lambda url: {
            'http://example.com/feed1': mock_feed1,
            'http://example.com/feed2': mock_feed2
        }.get(url, MagicMock(bozo=1, bozo_exception="Invalid feed"))
        yield mock_parse

def test_force_mark_all_as_posted_excludes_new_articles(mock_config_manager, mock_feedparser, tmp_path):
    """ force_mark_all_as_postedでマークされた記事がget_new_articlesで除外されることをテスト """
    # --- Arrange ---
    am = ArticleManager(mock_config_manager, feed_name='feed1')

    # force_mark_all_as_postedを実行して、すべての記事を投稿済みとしてマーク
    am.force_mark_all_as_posted()

    # 期待される投稿済みURLリスト
    expected_forced_posted_urls = {
        "http://example.com/feed1/article1",
        "http://example.com/feed1/article2",
        "http://example.com/feed2/article1"
    }

    # 既存の保存済み記事ファイルを作成（空または関連しない記事）
    saved_articles_path = tmp_path / "test_articles.json"
    with open(saved_articles_path, 'w', encoding='utf-8') as f:
        json.dump([], f)

    # 最新記事として、force_mark_all_as_postedでマークされた記事を含むリストを用意
    latest_articles = [
        {'title': 'Article 1 from Feed 1', 'link': 'http://example.com/feed1/article1', 'published_parsed': datetime(2024, 1, 1, 10, 0, 0, tzinfo=timezone.utc)},
        {'title': 'Article 2 from Feed 1', 'link': 'http://example.com/feed1/article2', 'published_parsed': datetime(2024, 1, 1, 11, 0, 0, tzinfo=timezone.utc)},
        {'title': 'Brand New Article', 'link': 'http://example.com/feed1/new_article', 'published_parsed': datetime(2024, 1, 1, 13, 0, 0, tzinfo=timezone.utc)}
    ]

    # --- Act ---
    # get_new_articlesを呼び出す
    new_articles = am.get_new_articles(latest_articles, [], debug=False)

    # --- Assert ---
    # force_mark_all_as_postedでマークされた記事が除外され、新しい記事のみが返されることを確認
    assert len(new_articles) == 1
    assert new_articles[0]['link'] == "http://example.com/feed1/new_article"

def test_force_mark_all_as_posted_writes_to_file(mock_config_manager, mock_feedparser, tmp_path):
    """ force_mark_all_as_postedが正しくJSONファイルに書き込むことをテスト """
    # --- Arrange ---
    am = ArticleManager(mock_config_manager, feed_name='feed1')
    data_file_path = tmp_path / "test_articles.json"
    ArticleManager.DATA_FILE = data_file_path

    # --- Act ---
    result = am.force_mark_all_as_posted()

    # --- Assert ---
    assert result['status'] == 'success'
    assert result['processed_articles'] == 3 # 2つのフィードから合計3つの記事

    with open(data_file_path, 'r', encoding='utf-8') as f:
        data = json.load(f)
    
    assert '__forced_posted_urls__' in data
    forced_urls = set(data['__forced_posted_urls__'])
    assert "http://example.com/feed1/article1" in forced_urls
    assert "http://example.com/feed1/article2" in forced_urls
    assert "http://example.com/feed2/article1" in forced_urls
