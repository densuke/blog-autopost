import pytest
from unittest.mock import patch, mock_open, MagicMock
import os
import json
import sys

# テスト対象のモジュールをインポート
from src.config_manager import load_config
from src.article_manager import get_latest_articles, load_saved_articles, save_articles, get_new_articles


@patch('builtins.open', new_callable=mock_open, read_data='blog:\n  feed_url: "http://test.com/feed"\nsns:\n  x:\n    consumer_key: "key"\n    consumer_secret: "secret"\n    access_token: "token"\n    access_token_secret: "token_secret"\n')
def test_load_config(mock_file):
    config = load_config("config.yml")
    assert config['blog']['feed_url'] == "http://test.com/feed"
    assert config['sns']['x']['consumer_key'] == "key"

@patch('feedparser.parse')
def test_get_latest_articles(mock_feedparser_parse):
    # モックのフィードデータ
    mock_feedparser_parse.return_value = type('obj', (object,), {'entries': [
        type('obj', (object,), {'title': 'Title 1', 'link': 'http://link1.com'}),
        type('obj', (object,), {'title': 'Title 2', 'link': 'http://link2.com'}),
    ]})

    articles = get_latest_articles("http://test.com/feed")
    assert len(articles) == 2
    assert articles[0]['title'] == "Title 1"
    assert articles[1]['link'] == "http://link2.com"

@patch('os.path.exists', return_value=True)
@patch('builtins.open', new_callable=mock_open, read_data=json.dumps([{"title": "Old 1", "link": "http://old1.com"}]))
def test_load_saved_articles_exists(mock_open_file, mock_exists):
    articles = load_saved_articles()
    assert len(articles) == 1
    assert articles[0]['title'] == "Old 1"

@patch('os.path.exists', return_value=False)
def test_load_saved_articles_not_exists(mock_exists):
    articles = load_saved_articles()
    assert articles == []

@patch('builtins.open', new_callable=mock_open)
@patch('json.dump')
def test_save_articles(mock_json_dump, mock_open_file):
    articles = [{"title": "New 1", "link": "http://new1.com"}]
    save_articles(articles)
    mock_open_file.assert_called_once_with(os.path.join("data", "articles.json"), 'w', encoding='utf-8')
    mock_json_dump.assert_called_once_with(articles, mock_open_file(), ensure_ascii=False, indent=4)

def test_get_new_articles():
    latest = [
        {"title": "Article A", "link": "http://a.com"},
        {"title": "Article B", "link": "http://b.com"},
        {"title": "Article C", "link": "http://c.com"},
    ]
    saved = [
        {"title": "Article A", "link": "http://a.com"},
        {"title": "Article D", "link": "http://d.com"},
    ]

    new_articles = get_new_articles(latest, saved);
    assert len(new_articles) == 2
    assert {"title": "Article B", "link": "http://b.com"} in new_articles
    assert {"title": "Article C", "link": "http://c.com"} in new_articles
    assert {"title": "Article A", "link": "http://a.com"} not in new_articles

def test_get_new_articles_no_new():
    latest = [
        {"title": "Article A", "link": "http://a.com"},
        {"title": "Article B", "link": "http://b.com"},
    ]
    saved = [
        {"title": "Article A", "link": "http://a.com"},
        {"title": "Article B", "link": "http://b.com"},
    ]

    new_articles = get_new_articles(latest, saved);
    assert len(new_articles) == 0

# main関数のテスト
@patch('src.main.load_config')
@patch('src.main.get_latest_articles')
@patch('src.main.load_saved_articles')
@patch('src.main.get_new_articles')
@patch('src.main.save_articles')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_debug_mode(mock_print, mock_load_plugins, mock_save_articles, mock_get_new_articles, mock_load_saved_articles, mock_get_latest_articles, mock_load_config):
    # モックの設定
    mock_load_config.return_value = {'blog': {'feed_url': 'http://test.com/feed'}}
    mock_get_latest_articles.return_value = [{'title': 'Test Article', 'link': 'http://test.com/article'}]
    mock_load_saved_articles.return_value = []
    mock_get_new_articles.return_value = [{'title': 'Test Article', 'link': 'http://test.com/article'}]

    # --debugフラグを付けてmainを実行
    with patch.object(sys, 'argv', ['src/main.py', '--debug']):
        from src.main import main
        main()

    # printがデバッグ情報を含めて呼ばれたか確認
    mock_print.assert_any_call("フィードURL: http://test.com/feed")
    mock_print.assert_any_call("フィードから取得した記事数: 1")

@patch('src.main.load_config')
@patch('src.main.get_latest_articles')
@patch('src.main.load_saved_articles')
@patch('src.main.get_new_articles')
@patch('src.main.save_articles')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_limit_option(mock_print, mock_load_plugins, mock_save_articles, mock_get_new_articles, mock_load_saved_articles, mock_get_latest_articles, mock_load_config):
    # モックの設定
    mock_load_config.return_value = {'blog': {'feed_url': 'http://test.com/feed'}}
    articles = [{'title': f'Article {i}', 'link': f'http://test.com/{i}'} for i in range(5)]
    mock_get_latest_articles.return_value = articles
    mock_load_saved_articles.return_value = []
    mock_get_new_articles.return_value = articles

    # --limit 2 を付けてmainを実行
    with patch.object(sys, 'argv', ['src/main.py', '--limit', '2', '--dry-run']):
        from src.main import main
        main()

    # 2つの記事だけが処理されたか確認
    mock_print.assert_any_call("直近の2個の記事のみを処理します。")
    mock_print.assert_any_call("[ドライラン] SNSに投稿: Title: Article 0, Link: http://test.com/0")
    mock_print.assert_any_call("[ドライラン] SNSに投稿: Title: Article 1, Link: http://test.com/1")
    
    # 3つ目の記事が処理されていないことを確認
    with pytest.raises(AssertionError):
        mock_print.assert_any_call("[ドライラン] SNSに投稿: Title: Article 2, Link: http://test.com/2")

@patch('src.main.load_config')
@patch('src.main.get_latest_articles')
@patch('src.main.load_saved_articles')
@patch('src.main.get_new_articles')
@patch('src.main.save_articles')
@patch('src.main.load_plugins')
@patch('builtins.print')
def test_main_dry_run_mode(mock_print, mock_load_plugins, mock_save_articles, mock_get_new_articles, mock_load_saved_articles, mock_get_latest_articles, mock_load_config):
    # モックの設定
    mock_load_config.return_value = {'blog': {'feed_url': 'http://test.com/feed'}, 'sns': {'x': {}}}
    mock_get_latest_articles.return_value = [{'title': 'Test Article', 'link': 'http://test.com/article'}]
    mock_load_saved_articles.return_value = []
    mock_get_new_articles.return_value = [{'title': 'Test Article', 'link': 'http://test.com/article'}]

    # --dry-runフラグを付けてmainを実行
    with patch.object(sys, 'argv', ['src/main.py', '--dry-run']):
        from src.main import main
        main()

    # ドライランのメッセージが表示されたか確認
    mock_print.assert_any_call("[ドライラン] SNSに投稿: Title: Test Article, Link: http://test.com/article")
    # load_pluginsが呼ばれていないことを確認
    mock_load_plugins.assert_not_called()
