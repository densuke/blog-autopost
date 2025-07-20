from atproto import Client, client_utils, models
from typing import List, Optional
import re
from . import SocialMediaPlugin

class Bluesky(SocialMediaPlugin):
    def __init__(self, identifier, password):
        self.sns_type = "bluesky"
        self.client = Client()
        # Blueskyにログイン
        self.client.login(identifier, password)

    def post(self, optimized_text: str, media_files: Optional[List[str]] = None):
        """
        Blueskyに投稿します
        
        Args:
            optimized_text: 投稿テキスト
            media_files: 添付するメディアファイルのパスリスト（画像のみ）
        """
        # 最適化済みテキストから手動でリンクを抽出
        url_pattern = r'https?://[^\s]+'
        urls = re.findall(url_pattern, optimized_text)
        
        # テキスト部分を構築
        if urls:
            # 最後のURLをリンクとして扱い、テキストから除去
            link = urls[-1]
            text_part = optimized_text.replace(link, '').strip()
            text_builder = client_utils.TextBuilder().text(f"{text_part} ").link(link, link)
        else:
            # URLがない場合はそのまま投稿
            text_builder = optimized_text
        
        # 画像の処理
        images = []
        if media_files:
            for media_path in media_files:
                try:
                    # 画像をアップロード
                    with open(media_path, 'rb') as f:
                        img_data = f.read()
                    
                    # MIMEタイプを推定
                    if media_path.lower().endswith(('.jpg', '.jpeg')):
                        mime_type = 'image/jpeg'
                    elif media_path.lower().endswith('.png'):
                        mime_type = 'image/png'
                    elif media_path.lower().endswith('.gif'):
                        mime_type = 'image/gif'
                    elif media_path.lower().endswith('.webp'):
                        mime_type = 'image/webp'
                    else:
                        mime_type = 'image/jpeg'  # デフォルト
                    
                    # Blueskyに画像をアップロード
                    upload_response = self.client.upload_blob(img_data)
                    
                    # 画像情報を作成
                    image = models.AppBskyEmbedImages.Image(
                        alt='',  # alt textは空文字
                        image=upload_response.blob
                    )
                    images.append(image)
                    print(f"画像アップロード完了: {media_path}")
                    
                except Exception as e:
                    print(f"画像アップロードエラー: {media_path} - {e}")
                    # エラーが発生しても他の画像の処理を続行
        
        # 投稿パラメータを構築
        post_params = {'text': text_builder}
        
        if images:
            # 画像埋め込みを追加
            embed = models.AppBskyEmbedImages.Main(images=images)
            post_params['embed'] = embed
        
        # Blueskyに投稿
        response = self.client.send_post(**post_params)
        
        if images:
            print(f"Blueskyに投稿しました（画像 {len(images)}件添付）: {response.uri}")
        else:
            print(f"Blueskyに投稿しました: {response.uri}")
