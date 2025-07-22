import feedparser
import json
import os
import requests
from bs4 import BeautifulSoup
from urllib.parse import urljoin, urlparse
from PIL import Image
import io
from datetime import datetime, timezone
import time
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
        
        articles = []
        for entry in feed.entries:
            # 日付をパースして比較可能な形式に変換
            published_time = self._parse_published_date(entry)
            
            article = {
                'title': entry.title, 
                'link': entry.link, 
                'published': entry.published,  # 元の文字列も保持
                'published_parsed': published_time,  # パース済み日付
                '_feed_entry': entry  # フィードエントリーを一時保存
            }
            
            if debug:
                print(f"記事: {entry.title[:50]}... 日付: {entry.published}")
            
            articles.append(article)
            
        # パース済み日付でソート（新しい順）
        return sorted(articles, key=lambda x: x['published_parsed'], reverse=True)

    def load_saved_articles(self):
        if not os.path.exists(DATA_FILE):
            return []
        with open(DATA_FILE, 'r', encoding='utf-8') as f:
            return json.load(f)

    def save_articles(self, articles):
        # 保存前に_feed_entryとpublished_parsedを削除（JSONシリアライズエラー回避）
        clean_articles = []
        for article in articles:
            clean_article = {k: v for k, v in article.items() if k not in ('_feed_entry', 'published_parsed')}
            clean_articles.append(clean_article)
        
        os.makedirs(os.path.dirname(DATA_FILE), exist_ok=True)
        with open(DATA_FILE, 'w', encoding='utf-8') as f:
            json.dump(clean_articles, f, indent=4, ensure_ascii=False)

    def get_new_articles(self, latest_articles, saved_articles, debug=False, limit=None):
        saved_links = {article['link'] for article in saved_articles}
        new_articles = [article for article in latest_articles if article['link'] not in saved_links]
        
        # limit指定がある場合は制限を適用
        if limit:
            new_articles = new_articles[:limit]
        
        # 新着記事のみに画像抽出を実行
        for article in new_articles:
            if debug:
                print(f"新着記事の画像を取得中: {article['title']}")
            if '_feed_entry' in article:
                article['image'] = self._extract_article_image(article['_feed_entry'], debug=debug)
                # 一時保存用のフィードエントリーを削除
                del article['_feed_entry']
        
        return new_articles

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
    
    def _extract_article_image(self, entry, debug=False):
        """
        記事から画像を抽出します
        
        Args:
            entry: feedparser entry オブジェクト
            debug (bool): デバッグ出力を行うかどうか
            
        Returns:
            str or None: 画像URL、見つからない場合はNone
        """
        image_settings = self.config_manager.get_image_settings()
        
        if not image_settings or not image_settings.get('enable_link_cards', False):
            return None
            
        strategies = image_settings.get('image_strategy', ['featured_image', 'og_image', 'default'])
        
        for strategy in strategies:
            try:
                if strategy == 'featured_image':
                    image_url = self._get_featured_image(entry)
                elif strategy == 'first_content_image':
                    image_url = self._get_first_content_image(entry)
                elif strategy == 'og_image':
                    image_url = self._get_og_image(entry.link)
                elif strategy == 'default':
                    image_url = image_settings.get('default_image')
                else:
                    continue
                    
                if debug:
                    print(f"[DEBUG] 戦略 '{strategy}' で取得した画像URL: {image_url}")
                
                if image_url and self._is_valid_image(image_url, image_settings):
                    if debug:
                        print(f"記事 '{entry.title[:30]}...' の画像を取得しました ({strategy}): {image_url}")
                    return image_url
                    
            except Exception as e:
                if debug:
                    print(f"画像取得エラー ({strategy}): {e}")
                continue
                
        return None
    
    def _get_featured_image(self, entry):
        """
        RSS/AtomフィードからfeaturedImage（enclosureやmedia:content）を取得
        """
        # enclosureタグから画像を検索
        if hasattr(entry, 'enclosures') and entry.enclosures:
            for enclosure in entry.enclosures:
                if enclosure.type and 'image' in enclosure.type:
                    return enclosure.href
        
        # media:contentタグから画像を検索
        if hasattr(entry, 'media_content') and entry.media_content:
            for media in entry.media_content:
                if media.get('type') and 'image' in media['type']:
                    return media.get('url')
                    
        return None
    
    def _get_first_content_image(self, entry):
        """
        記事本文から最初の画像を取得
        """
        content = None
        if hasattr(entry, 'content') and entry.content:
            content = entry.content[0].value
            print(f"[DEBUG] RSSフィードのcontent: {content[:500]}...")
        elif hasattr(entry, 'summary'):
            content = entry.summary
            print(f"[DEBUG] RSSフィードのsummary: {content[:500]}...")
            
        if not content:
            print(f"[DEBUG] RSSフィードにcontent/summaryが存在しません")
            return None
            
        soup = BeautifulSoup(content, 'html.parser')
        img_tags = soup.find_all('img')
        print(f"[DEBUG] RSSコンテンツから検出された画像タグ数: {len(img_tags)}")
        
        for i, img in enumerate(img_tags):
            src = img.get('src')
            print(f"[DEBUG] 画像タグ{i+1}: {img}")
            if src:
                # 相対URLを絶対URLに変換
                absolute_url = urljoin(entry.link, src)
                print(f"[DEBUG] 変換された絶対URL: {absolute_url}")
                return absolute_url
                
        print(f"[DEBUG] RSSコンテンツに有効な画像が見つかりませんでした")
        return None
    
    def _get_og_image(self, article_url):
        """
        記事ページからOGP画像を取得
        """
        try:
            print(f"[DEBUG] OGP画像取得のため記事ページにアクセス: {article_url}")
            response = requests.get(article_url, timeout=10, headers={
                'User-Agent': 'Mozilla/5.0 (compatible; Blog-AutoPost/1.0)'
            })
            response.raise_for_status()
            print(f"[DEBUG] ページアクセス成功、HTMLサイズ: {len(response.text)} 文字")
            
            soup = BeautifulSoup(response.text, 'html.parser')
            
            # OGP画像を検索
            og_image = soup.find('meta', property='og:image')
            print(f"[DEBUG] OGP画像タグ: {og_image}")
            
            if og_image and og_image.get('content'):
                og_image_url = urljoin(article_url, og_image['content'])
                print(f"[DEBUG] OGP画像URL: {og_image_url}")
                return og_image_url
            
            # OGPが見つからない場合、記事内の最初の画像を探してみる
            print(f"[DEBUG] OGP画像が見つからないため、記事内の画像を検索")
            img_tags = soup.find_all('img')
            print(f"[DEBUG] 記事ページ内の画像タグ数: {len(img_tags)}")
            
            for i, img in enumerate(img_tags):
                src = img.get('src')
                if src:
                    print(f"[DEBUG] 記事内画像{i+1}: src='{src}'")
                    absolute_url = urljoin(article_url, src)
                    print(f"[DEBUG] 変換後の絶対URL: {absolute_url}")
                    # 最初に見つかった画像を返す（これは本来first_content_imageの役割だが、補助として）
                    return absolute_url
                
        except Exception as e:
            print(f"[DEBUG] OGP画像取得エラー: {e}")
            
        return None
    
    def _is_valid_image(self, image_url, image_settings):
        """
        画像URLが有効かどうかをチェック
        
        Args:
            image_url (str): 画像URL
            image_settings (dict): 画像設定
            
        Returns:
            bool: 有効な画像の場合True
        """
        if not image_url:
            return False
            
        # 除外ドメインチェック
        filters = image_settings.get('image_filters', {})
        exclude_domains = filters.get('exclude_domains', [])
        
        parsed_url = urlparse(image_url)
        domain = parsed_url.netloc.lower()
        
        for exclude_domain in exclude_domains:
            if exclude_domain.lower() in domain:
                return False
                
        # 画像サイズチェック（オプション）
        min_width = filters.get('min_width', 0)
        min_height = filters.get('min_height', 0)
        
        if min_width > 0 or min_height > 0:
            try:
                response = requests.head(image_url, timeout=5, headers={
                    'User-Agent': 'Mozilla/5.0 (compatible; Blog-AutoPost/1.0)'
                })
                if response.status_code == 200:
                    # 実際の画像サイズをチェック（必要な場合のみ）
                    if 'image' in response.headers.get('content-type', '').lower():
                        return True
            except Exception:
                pass
                
        return True
    
    def _parse_published_date(self, entry):
        """
        フィードエントリーから日付を解析します
        
        Args:
            entry: feedparser entry オブジェクト
            
        Returns:
            datetime: パース済み日付、失敗時は Unix epoch
        """
        # feedparserが自動でパースしたpublished_parsedを優先使用
        if hasattr(entry, 'published_parsed') and entry.published_parsed:
            try:
                # UTC基準でタイムゾーン情報を付与
                return datetime.fromtimestamp(time.mktime(entry.published_parsed), tz=timezone.utc)
            except (ValueError, TypeError, OverflowError):
                pass
        
        # published_parsedが無い場合は文字列をパース
        if hasattr(entry, 'published'):
            try:
                # RFC 2822形式などの一般的な日付形式をパース
                return datetime.strptime(entry.published, '%a, %d %b %Y %H:%M:%S %z')
            except ValueError:
                try:
                    # ISO 8601形式
                    return datetime.fromisoformat(entry.published.replace('Z', '+00:00'))
                except ValueError:
                    pass
        
        # 日付が取得できない場合はUnix epochを返す（古い日付として扱われる）
        return datetime.fromtimestamp(0, tz=timezone.utc)
