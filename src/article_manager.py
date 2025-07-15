import feedparser
import json
import os
import requests

DATA_DIR = "data"
ARTICLES_FILE = os.path.join(DATA_DIR, "articles.json")

def get_latest_articles(feed_url, debug=False):
    """
    指定されたRSS/Atomフィードから最新の記事リストを取得します。

    Args:
        feed_url (str): RSS/AtomフィードのURL。
        debug (bool): デバッグ情報を表示するかどうか。

    Returns:
        list: 記事のリスト。各記事はタイトルとリンクを持つ辞書です。
    """
    if debug:
        print(f"フィードを取得中: {feed_url}")
        
        # まずHTTPリクエストで内容を確認
        try:
            response = requests.get(feed_url, timeout=10)
            print(f"HTTPステータス: {response.status_code}")
            print(f"Content-Type: {response.headers.get('Content-Type', 'N/A')}")
            
            content = response.text
            print(f"レスポンス内容の先頭200文字:")
            print(content[:200])
            print("---")
            
            if response.status_code != 200:
                print(f"HTTPエラー: {response.status_code}")
                return []
                
            # XMLかどうかチェック
            if not content.strip().startswith('<?xml') and not content.strip().startswith('<'):
                print("警告: レスポンスがXMLフォーマットではありません")
                return []
                
        except requests.RequestException as e:
            print(f"HTTPリクエストエラー: {e}")
            return []
    
    feed = feedparser.parse(feed_url)
    
    if debug:
        print(f"feedparserのステータス: {feed.status if hasattr(feed, 'status') else 'N/A'}")
        print(f"フィードのタイトル: {feed.feed.title if hasattr(feed, 'feed') and hasattr(feed.feed, 'title') else 'N/A'}")
        print(f"エントリー数: {len(feed.entries)}")
        if hasattr(feed, 'bozo') and feed.bozo:
            print(f"フィードパースエラー: {feed.bozo_exception}")
    
    articles = []
    for entry in feed.entries:
        articles.append({
            'title': entry.title,
            'link': entry.link
        })
    return articles

def load_saved_articles():
    """
    保存された記事リストを読み込みます。

    Returns:
        list: 保存された記事のリスト。
    """
    if os.path.exists(ARTICLES_FILE):
        with open(ARTICLES_FILE, 'r', encoding='utf-8') as f:
            return json.load(f)
    return []

def save_articles(articles):
    """
    記事リストを保存します。

    Args:
        articles (list): 保存する記事のリスト。
    """
    with open(ARTICLES_FILE, 'w', encoding='utf-8') as f:
        json.dump(articles, f, ensure_ascii=False, indent=4)

def get_new_articles(latest_articles, saved_articles):
    """
    新しい記事を検出します。

    Args:
        latest_articles (list): 最新の記事リスト。
        saved_articles (list): 保存された記事リスト。

    Returns:
        list: 新しい記事のリスト。
    """
    new_articles = []
    saved_links = {article['link'] for article in saved_articles}
    for article in latest_articles:
        if article['link'] not in saved_links:
            new_articles.append(article)
    return new_articles
