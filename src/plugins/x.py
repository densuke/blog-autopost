from tweepy import Client, API, OAuth1UserHandler
from typing import List, Optional
from . import SocialMediaPlugin

class X(SocialMediaPlugin):
    def __init__(self, consumer_key, consumer_secret, access_token, access_token_secret):
        self.sns_type = "x"
        
        # v2 Client (投稿用)
        self.client = Client(
            consumer_key=consumer_key,
            consumer_secret=consumer_secret,
            access_token=access_token,
            access_token_secret=access_token_secret
        )
        
        # v1.1 API (メディアアップロード用)
        auth = OAuth1UserHandler(
            consumer_key=consumer_key,
            consumer_secret=consumer_secret,
            access_token=access_token,
            access_token_secret=access_token_secret
        )
        self.api = API(auth)

    def post(self, text_or_title, link_or_media=None, **kwargs):
        """
        Xに投稿します
        
        Args:
            text_or_title: 投稿テキストまたはタイトル
            link_or_media: リンクURLまたはメディアファイルのパスリスト
        """
        # 後方互換性: (title, link)形式と(optimized_text, media_files)形式を両方サポート
        if isinstance(link_or_media, str) and link_or_media.startswith('http'):
            # 従来の (title, link) 形式
            optimized_text = f"{text_or_title} {link_or_media}"
            media_files = None
        elif isinstance(link_or_media, list):
            # 新しい (optimized_text, media_files) 形式
            optimized_text = text_or_title
            media_files = link_or_media
        else:
            # optimized_textのみの場合
            optimized_text = text_or_title
            media_files = link_or_media if link_or_media else None
        
        media_ids = []
        
        if media_files:
            for media_path in media_files:
                try:
                    # v1.1 APIでメディアをアップロード
                    media = self.api.media_upload(media_path)
                    media_ids.append(media.media_id)
                except Exception as e:
                    print(f"メディアアップロードエラー: {media_path} - {e}")
                    # エラーが発生しても他のメディアの処理を続行
        
        # v2 APIでツイートを投稿
        tweet_params = {"text": optimized_text}
        if media_ids:
            tweet_params["media_ids"] = media_ids
        
        response = self.client.create_tweet(**tweet_params)
        
        if media_ids:
            print(f"Xに投稿しました（メディア {len(media_ids)}件添付）: {response.data['id']}")
        else:
            print(f"Xに投稿しました: {response.data['id']}")
