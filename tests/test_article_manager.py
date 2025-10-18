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

def test_force_mark_all_as_posted_writes_to_file(mock_config_manager, mock_feedparser, tmp_path, monkeypatch):
    """ force_mark_all_as_postedが正しくJSONファイルに書き込むことをテスト """
    # --- Arrange ---
    data_file_path = str(tmp_path / "test_articles.json")
    # モジュールレベルの DATA_FILE をパッチ
    monkeypatch.setattr('src.article_manager.DATA_FILE', data_file_path)
    
    am = ArticleManager(mock_config_manager, feed_name='feed1')

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


def test_get_latest_articles_success(mock_config_manager, mock_feedparser):
    """get_latest_articles が RSS フィードから記事を正しく取得することをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager, feed_name='feed1')
    
    # --- Act ---
    articles = am.get_latest_articles(debug=False)
    
    # --- Assert ---
    assert len(articles) == 2
    assert articles[0]['title'] == "Article 2 from Feed 1"  # 新しい順
    assert articles[1]['title'] == "Article 1 from Feed 1"
    assert articles[0]['link'] == "http://example.com/feed1/article2"
    assert articles[1]['link'] == "http://example.com/feed1/article1"

def test_get_latest_articles_invalid_feed(mock_config_manager, mock_feedparser):
    """get_latest_articles が無効なフィードを処理することをテスト"""
    # --- Arrange ---
    mock_config_manager.get_feed_config_by_name.return_value = {
        'name': 'invalid_feed',
        'feed_url': 'http://example.com/invalid'
    }
    am = ArticleManager(mock_config_manager, feed_name='invalid_feed')
    
    # --- Act ---
    articles = am.get_latest_articles(debug=False)
    
    # --- Assert ---
    assert articles == []

def test_load_saved_articles_empty(mock_config_manager, tmp_path, monkeypatch):
    """load_saved_articles が空のファイルから読み込むことをテスト"""
    # --- Arrange ---
    data_file_path = str(tmp_path / "test_articles.json")
    monkeypatch.setattr('src.article_manager.DATA_FILE', data_file_path)
    
    # 空のJSONファイルを作成
    os.makedirs(os.path.dirname(data_file_path), exist_ok=True)
    with open(data_file_path, 'w', encoding='utf-8') as f:
        json.dump([], f)
    
    am = ArticleManager(mock_config_manager)
    
    # --- Act ---
    articles = am.load_saved_articles()
    
    # --- Assert ---
    assert articles == []

def test_load_saved_articles_with_data(mock_config_manager, tmp_path, monkeypatch):
    """load_saved_articles が保存済みの記事を正しく読み込むことをテスト"""
    # --- Arrange ---
    data_file_path = str(tmp_path / "test_articles.json")
    monkeypatch.setattr('src.article_manager.DATA_FILE', data_file_path)
    
    # テスト用のJSONファイルを作成
    os.makedirs(os.path.dirname(data_file_path), exist_ok=True)
    test_data = [
        {'title': 'Saved Article 1', 'link': 'http://example.com/saved1'},
        {'title': 'Saved Article 2', 'link': 'http://example.com/saved2'}
    ]
    with open(data_file_path, 'w', encoding='utf-8') as f:
        json.dump(test_data, f)
    
    am = ArticleManager(mock_config_manager)
    
    # --- Act ---
    articles = am.load_saved_articles()
    
    # --- Assert ---
    assert len(articles) == 2
    assert articles[0]['title'] == 'Saved Article 1'
    assert articles[1]['link'] == 'http://example.com/saved2'

def test_save_articles(mock_config_manager, tmp_path, monkeypatch):
    """save_articles が記事を正しくファイルに保存することをテスト"""
    # --- Arrange ---
    data_file_path = str(tmp_path / "test_articles.json")
    monkeypatch.setattr('src.article_manager.DATA_FILE', data_file_path)
    
    os.makedirs(os.path.dirname(data_file_path), exist_ok=True)
    
    am = ArticleManager(mock_config_manager)
    articles_to_save = [
        {'title': 'Article 1', 'link': 'http://example.com/article1'},
        {'title': 'Article 2', 'link': 'http://example.com/article2'}
    ]
    
    # --- Act ---
    am.save_articles(articles_to_save)
    
    # --- Assert ---
    with open(data_file_path, 'r', encoding='utf-8') as f:
        saved_data = json.load(f)
    
    assert len(saved_data) == 2
    assert saved_data[0]['title'] == 'Article 1'
    assert saved_data[1]['link'] == 'http://example.com/article2'

def test_get_new_articles_filtering(mock_config_manager, mock_feedparser):
    """get_new_articles が正しく新着記事をフィルタリングすることをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager, feed_name='feed1')
    
    latest_articles = [
        {'title': 'New Article', 'link': 'http://example.com/new', 'published_parsed': datetime(2024, 1, 2, 10, 0, 0, tzinfo=timezone.utc)},
        {'title': 'Article 1 from Feed 1', 'link': 'http://example.com/feed1/article1', 'published_parsed': datetime(2024, 1, 1, 10, 0, 0, tzinfo=timezone.utc)},
    ]
    
    saved_articles = [
        {'title': 'Article 1 from Feed 1', 'link': 'http://example.com/feed1/article1'}
    ]
    
    # --- Act ---
    new_articles = am.get_new_articles(latest_articles, saved_articles, debug=False)
    
    # --- Assert ---
    assert len(new_articles) == 1
    assert new_articles[0]['link'] == 'http://example.com/new'

def test_create_post_text(mock_config_manager, mock_feedparser):
    """create_post_text が投稿テキストを正しく生成することをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager, feed_name='feed1')
    title = "Test Article Title"
    link = "http://example.com/article"
    
    # --- Act ---
    post_text = am.create_post_text(title, link, 'x')
    
    # --- Assert ---
    assert title in post_text
    assert link in post_text


def test_is_valid_image_valid_url(mock_config_manager):
    """_is_valid_image が有効な画像URLを判定することをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager)
    image_url = "http://example.com/image.jpg"
    image_settings = {
        'image_filters': {
            'exclude_domains': ['spam.com', 'ads.com'],
            'min_width': 0,
            'min_height': 0
        }
    }
    
    # --- Act ---
    result = am._is_valid_image(image_url, image_settings)
    
    # --- Assert ---
    assert result is True

def test_is_valid_image_excluded_domain(mock_config_manager):
    """_is_valid_image が除外ドメインを正しく処理することをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager)
    image_url = "http://ads.com/banner.jpg"
    image_settings = {
        'image_filters': {
            'exclude_domains': ['spam.com', 'ads.com'],
            'min_width': 0,
            'min_height': 0
        }
    }
    
    # --- Act ---
    result = am._is_valid_image(image_url, image_settings)
    
    # --- Assert ---
    assert result is False

def test_is_valid_image_empty_url(mock_config_manager):
    """_is_valid_image が空のURLを処理することをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager)
    image_url = ""
    image_settings = {'image_filters': {}}
    
    # --- Act ---
    result = am._is_valid_image(image_url, image_settings)
    
    # --- Assert ---
    assert result is False

def test_parse_published_date_valid_date(mock_config_manager):
    """_parse_published_date が有効な日付を解析することをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager)
    
    # datetime オブジェクトをモック
    mock_entry = MagicMock()
    mock_entry.published_parsed = datetime(2024, 1, 15, 12, 30, 0, tzinfo=timezone.utc).timetuple()
    
    # --- Act ---
    result = am._parse_published_date(mock_entry)
    
    # --- Assert ---
    assert result is not None
    assert isinstance(result, datetime)

def test_parse_published_date_no_date(mock_config_manager):
    """_parse_published_date が日付がない場合を処理することをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager)
    
    mock_entry = MagicMock()
    mock_entry.published_parsed = None
    mock_entry.published = "Mon, 01 Jan 2024 10:00:00 +0000"
    
    # --- Act ---
    result = am._parse_published_date(mock_entry)
    
    # --- Assert ---
    assert result is not None
    assert isinstance(result, datetime)

def test_get_featured_image_with_enclosure(mock_config_manager):
    """_get_featured_image がenclosureから画像を取得することをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager)
    
    mock_enclosure = MagicMock()
    mock_enclosure.type = 'image/jpeg'
    mock_enclosure.href = 'http://example.com/image.jpg'
    
    mock_entry = MagicMock()
    mock_entry.enclosures = [mock_enclosure]
    mock_entry.media_content = []
    
    # --- Act ---
    result = am._get_featured_image(mock_entry)
    
    # --- Assert ---
    assert result == 'http://example.com/image.jpg'

def test_get_featured_image_no_image(mock_config_manager):
    """_get_featured_image が画像がない場合を処理することをテスト"""
    # --- Arrange ---
    am = ArticleManager(mock_config_manager)
    
    mock_entry = MagicMock()
    mock_entry.enclosures = []
    mock_entry.media_content = []
    
    # --- Act ---
    result = am._get_featured_image(mock_entry)
    
    # --- Assert ---
    assert result is None

def test_extract_article_image_disabled(mock_config_manager):
    """_extract_article_image がリンクカード無効時にNoneを返すことをテスト"""
    # --- Arrange ---
    mock_config_manager.get_image_settings.return_value = {
        'enable_link_cards': False
    }
    am = ArticleManager(mock_config_manager)
    
    mock_entry = MagicMock()
    
    # --- Act ---
    result = am._extract_article_image(mock_entry)
    
    # --- Assert ---
    assert result is None

def test_extract_article_image_default_strategy(mock_config_manager):
    """_extract_article_image がデフォルト画像戦略を使用することをテスト"""
    # --- Arrange ---
    mock_config_manager.get_image_settings.return_value = {
        'enable_link_cards': True,
        'image_strategy': ['default'],
        'default_image': 'http://example.com/default.jpg',
        'image_filters': {}
    }
    am = ArticleManager(mock_config_manager)
    
    mock_entry = MagicMock()
    
    # --- Act ---
    result = am._extract_article_image(mock_entry)
    
    # --- Assert ---
    assert result == 'http://example.com/default.jpg'
