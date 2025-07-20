import feedparser
import json
import os
from .config_manager import ConfigManager
from .text_optimizer import TextOptimizer

DATA_FILE = "data/articles.json"

class ArticleManager:
    def __init__(self, config_manager: ConfigManager):
        self.config_manager = config_manager
        self.feed_url = self.config_manager.get_feed_url()
        self.text_optimizer = TextOptimizer(config_manager.config)

    def get_latest_articles(self, debug=False):
        if debug:
            print(f"フィードを解析中: {self.feed_url}")
        feed = feedparser.parse(self.feed_url)
        if feed.bozo:
            if debug:
                print(f"フィードの解析に失敗しました: {feed.bozo_exception}")
            return []
        return sorted([
            {'title': entry.title, 'link': entry.link, 'published': entry.published}
            for entry in feed.entries
        ], key=lambda x: x['published'], reverse=True)

    def load_saved_articles(self):
        if not os.path.exists(DATA_FILE):
            return []
        with open(DATA_FILE, 'r', encoding='utf-8') as f:
            return json.load(f)

    def save_articles(self, articles):
        os.makedirs(os.path.dirname(DATA_FILE), exist_ok=True)
        with open(DATA_FILE, 'w', encoding='utf-8') as f:
            json.dump(articles, f, indent=4, ensure_ascii=False)

    def get_new_articles(self, latest_articles, saved_articles):
        saved_links = {article['link'] for article in saved_articles}
        return [article for article in latest_articles if article['link'] not in saved_links]

    def create_post_text(self, title: str, link: str, sns_type: str) -> str:
        """
        SNS別に最適化されたポストテキストを作成します
        
        Args:
            title (str): 記事タイトル
            link (str): 記事URL
            sns_type (str): SNS種別
            
        Returns:
            str: 最適化されたポストテキスト
        """
        announcement = self.config_manager.get_announcement_text()
        return self.text_optimizer.optimize_text(title, link, sns_type, announcement)
