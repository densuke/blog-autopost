from atproto import Client, client_utils, models
from typing import List, Optional, Dict, Any
import re
import requests
from bs4 import BeautifulSoup
from urllib.parse import urljoin
from . import SocialMediaPlugin
from ..image_resizer import create_image_resizer

class Bluesky(SocialMediaPlugin):
    def __init__(self, identifier, password, config=None):
        self.sns_type = "bluesky"
        self.client = Client()
        self.config = config or {}
        # Blueskyにログイン
        self.client.login(identifier, password)

    def supports_rich_content(self) -> bool:
        """リッチコンテンツ（リンクカード）をサポートする"""
        return True
    
    def post(self, optimized_text: str, media_files: Optional[List[str]] = None, **kwargs: Any):
        """
        Blueskyに投稿します
        
        Args:
            optimized_text: 投稿テキスト
            media_files: 添付するメディアファイルのパスリスト（画像のみ）
            **kwargs: 追加パラメータ（article_data: 記事データなど）
        """
        # kwargsから記事データを取得
        article_data = kwargs.get('article_data')
        debug = kwargs.get('debug', False)
        
        # 最適化済みテキストから手動でリンクを抽出
        url_pattern = r'https?://[^\s]+'
        urls = re.findall(url_pattern, optimized_text)
        
        # リンクカード機能が有効かチェック
        image_settings = self.config.get('blog', {}).get('image_settings', {})
        enable_link_cards = image_settings.get('enable_link_cards', False)
        self._debug_print(f"リンクカード設定: {enable_link_cards}, 設定内容: {image_settings}", debug)
        self._debug_print(f"article_data存在: {bool(article_data)}", debug)
        if article_data:
            self._debug_print(f"article_dataキー: {list(article_data.keys())}", debug)
        
        # テキスト部分を構築
        link = None
        if urls:
            # 最後のURLをリンクとして扱い、テキストから除去
            link = urls[-1]
            text_part = optimized_text.replace(link, '').strip()
            
            # リンクカードが有効な場合はテキストのみ、無効な場合は従来通り
            if enable_link_cards:
                text_builder = client_utils.TextBuilder().text(text_part)
            else:
                text_builder = client_utils.TextBuilder().text(f"{text_part} ").link(link, link)
        else:
            # URLがない場合はそのまま投稿
            text_builder = optimized_text
        
        # 画像の処理
        images = []
        if media_files:
            resizer = create_image_resizer(debug=debug)
            for media_path in media_files:
                try:
                    # 画像をリサイズしてからアップロード
                    with open(media_path, 'rb') as f:
                        original_data = f.read()
                    
                    self._debug_print(f"元画像サイズ: {len(original_data)} bytes ({media_path})", debug)
                    
                    # 画像リサイズ処理
                    resized_data = resizer.resize_image_data(original_data, 'bluesky')
                    
                    self._debug_print(f"リサイズ後サイズ: {len(resized_data)} bytes", debug)
                    
                    # Blueskyに画像をアップロード
                    upload_response = self.client.upload_blob(resized_data)
                    
                    # 画像情報を作成
                    image = models.AppBskyEmbedImages.Image(
                        alt='',  # alt textは空文字
                        image=upload_response.blob
                    )
                    images.append(image)
                    self._debug_print(f"画像アップロード完了: {media_path}", debug)
                    
                except Exception as e:
                    print(f"画像アップロードエラー: {media_path} - {e}")
                    # エラーが発生しても他の画像の処理を続行
        
        # 投稿パラメータを構築
        post_params = {'text': text_builder}
        
        # エンベッドの優先順位: 画像 > リンクカード
        if images:
            # 画像埋め込みを追加
            embed = models.AppBskyEmbedImages.Main(images=images)
            post_params['embed'] = embed
            self._debug_print("画像エンベッドを設定", debug)
        elif enable_link_cards and link and article_data:
            # リンクカードを作成
            self._debug_print("リンクカード機能が有効、作成を開始", debug)
            self._debug_print(f"リンクURL: {link}", debug)
            link_card = self._create_link_card(link, article_data, debug)
            if link_card:
                post_params['embed'] = link_card
                self._debug_print("リンクカードエンベッドを設定", debug)
            else:
                self._debug_print("リンクカード作成失敗、テキストのみ投稿", debug)
        else:
            self._debug_print(f"エンベッド条件不適合 - enable_link_cards:{enable_link_cards}, link:{bool(link)}, article_data存在:{bool(article_data)}", debug)
            if not enable_link_cards:
                self._debug_print("リンクカード機能が無効", debug)
            if not link:
                self._debug_print("リンクURLがない", debug)
            if not article_data:
                self._debug_print("article_dataがない", debug)
        
        try:
            # Blueskyに投稿
            response = self.client.send_post(**post_params)
            
            if images:
                print(f"Blueskyに投稿しました（画像 {len(images)}件添付）: {response.uri}")
            elif enable_link_cards and link:
                print(f"Blueskyにリンクカード付きで投稿しました: {response.uri}")
            else:
                print(f"Blueskyに投稿しました: {response.uri}")
        except Exception as e:
            print(f"Blueskyへの投稿中にエラー: {e}")
            raise
    
    def _create_link_card(self, url: str, article_data: Dict[str, Any], debug: bool = False) -> Optional[models.AppBskyEmbedExternal.Main]:
        """
        リンクカードを作成します
        
        Args:
            url: リンクURL
            article_data: 記事データ
            
        Returns:
            models.AppBskyEmbedExternal.Main or None: リンクカードエンベッド
        """
        try:
            self._debug_print(f"リンクカード作成開始: {url}", debug)
            
            # 記事のメタデータを取得
            title = article_data.get('title', '')
            description = self._get_description(url, article_data)
            image_url = article_data.get('image')
            
            self._debug_print(f"タイトル: {title}", debug)
            self._debug_print(f"説明文: {description[:100]}...", debug)
            self._debug_print(f"画像URL: {image_url}", debug)
            
            # サムネイル画像をアップロード（ある場合）
            thumb_blob = None
            if image_url:
                self._debug_print(f"画像アップロード開始: {image_url}", debug)
                thumb_blob = self._upload_thumbnail(image_url, debug)
                if thumb_blob:
                    self._debug_print("画像アップロード成功", debug)
                else:
                    self._debug_print("画像アップロード失敗", debug)
            else:
                self._debug_print("画像URLなし、画像なしでリンクカード作成", debug)
            
            # 外部エンベッドを作成
            external = models.AppBskyEmbedExternal.External(
                uri=url,
                title=title[:300] if title else 'ブログ記事',  # タイトル長制限
                description=description[:1000] if description else '',  # 説明文長制限
                thumb=thumb_blob
            )
            
            self._debug_print("リンクカード作成成功", debug)
            return models.AppBskyEmbedExternal.Main(external=external)
            
        except Exception as e:
            print(f"リンクカード作成エラー: {e}")
            return None
    
    def _get_description(self, url: str, article_data: Dict[str, Any]) -> str:
        """
        記事の説明文を取得します
        
        Args:
            url: 記事URL
            article_data: 記事データ
            
        Returns:
            str: 説明文
        """
        try:
            response = requests.get(url, timeout=10, headers={
                'User-Agent': 'Mozilla/5.0 (compatible; Blog-AutoPost/1.0)'
            })
            response.raise_for_status()
            
            # 文字コードの自動検出と適切なデコード
            html_content = self._decode_html_content(response)
            soup = BeautifulSoup(html_content, 'html.parser')
            
            # OGPの説明文を優先
            og_description = soup.find('meta', property='og:description')
            if og_description and og_description.get('content'):
                return og_description['content']
                
            # meta description
            meta_description = soup.find('meta', attrs={'name': 'description'})
            if meta_description and meta_description.get('content'):
                return meta_description['content']
                
            # 本文の最初の段落
            first_p = soup.find('p')
            if first_p and first_p.get_text(strip=True):
                return first_p.get_text(strip=True)[:200]
                
        except Exception as e:
            print(f"説明文取得エラー: {e}")
            
        return 'ブログ記事を投稿しました'
    
    def _decode_html_content(self, response) -> str:
        """
        HTMLコンテンツを適切な文字コードでデコードします
        
        Args:
            response: requestsのレスポンスオブジェクト
            
        Returns:
            str: デコードされたHTMLコンテンツ
        """
        # Content-Typeヘッダーからcharsetを取得
        content_type = response.headers.get('content-type', '').lower()
        
        # HTMLのmeta charsetタグから文字コードを検出
        html_bytes = response.content
        html_preview = html_bytes[:2048].decode('utf-8', errors='ignore').lower()
        
        # meta charset検出のパターン
        import re
        charset_patterns = [
            r'<meta[^>]+charset=["\']?([^"\'>\s]+)',
            r'<meta[^>]+content=["\'][^"\']*charset=([^"\'>\s]+)',
        ]
        
        detected_charset = None
        for pattern in charset_patterns:
            match = re.search(pattern, html_preview)
            if match:
                detected_charset = match.group(1).strip()
                break
        
        # 文字コード優先順位: meta charset > Content-Type > 自動検出
        encodings_to_try = []
        
        if detected_charset:
            encodings_to_try.append(detected_charset)
        
        if 'charset=' in content_type:
            charset_from_header = content_type.split('charset=')[1].split(';')[0].strip()
            if charset_from_header not in encodings_to_try:
                encodings_to_try.append(charset_from_header)
        
        # 日本語サイトでよく使われる文字コードを追加
        common_encodings = ['utf-8', 'shift_jis', 'euc-jp', 'iso-2022-jp', 'cp932']
        for encoding in common_encodings:
            if encoding not in encodings_to_try:
                encodings_to_try.append(encoding)
        
        # 各文字コードを順番に試行
        for encoding in encodings_to_try:
            try:
                return html_bytes.decode(encoding)
            except (UnicodeDecodeError, LookupError):
                continue
        
        # すべて失敗した場合はUTF-8でエラーを無視してデコード
        return html_bytes.decode('utf-8', errors='ignore')
    
    def _upload_thumbnail(self, image_url: str, debug: bool = False) -> Optional[models.ComAtprotoRepoUploadBlob.Response]:
        """
        サムネイル画像をBlueskyにアップロードします
        
        Args:
            image_url: 画像URL
            
        Returns:
            models.ComAtprotoRepoUploadBlob.Response or None: アップロードされたblob
        """
        try:
            # 画像をダウンロード
            response = requests.get(image_url, timeout=15, headers={
                'User-Agent': 'Mozilla/5.0 (compatible; Blog-AutoPost/1.0)'
            })
            response.raise_for_status()
            
            self._debug_print(f"元画像サイズ: {len(response.content)} bytes", debug)
            
            # 画像リサイズ処理
            resizer = create_image_resizer(debug=debug)
            resized_data = resizer.resize_image_data(response.content, 'bluesky')
            
            self._debug_print(f"リサイズ後サイズ: {len(resized_data)} bytes", debug)
            
            # Blueskyにアップロード
            upload_result = self.client.upload_blob(resized_data)
            return upload_result.blob
            
        except Exception as e:
            print(f"サムネイルアップロードエラー: {e}")
            return None