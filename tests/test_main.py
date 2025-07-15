import pytest
from unittest.mock import patch, mock_open
import os
import json

# テスト対象のモジュールをインポート
from src.main import (
    load_config,
    get_latest_articles,
    load_saved_articles,
    save_articles,
    get_new_articles,
)

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
