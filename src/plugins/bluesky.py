from atproto import Client, client_utils
from . import SocialMediaPlugin

class Bluesky(SocialMediaPlugin):
    def __init__(self, identifier, password):
        self.sns_type = "bluesky"
        self.client = Client()
        # Blueskyにログイン
        self.client.login(identifier, password)

    def post(self, optimized_text: str):
        # 最適化済みテキストから手動でリンクを抽出
        # 簡単な実装として、最後のhttps://で始まる文字列をリンクとして扱う
        import re
        url_pattern = r'https?://[^\s]+'
        urls = re.findall(url_pattern, optimized_text)
        
        if urls:
            # 最後のURLをリンクとして扱い、テキストから除去
            link = urls[-1]
            text_part = optimized_text.replace(link, '').strip()
            text_with_link = client_utils.TextBuilder().text(f"{text_part} ").link(link, link)
        else:
            # URLがない場合はそのまま投稿
            text_with_link = optimized_text
        
        # Blueskyに投稿
        response = self.client.send_post(text=text_with_link)
        print(f"Blueskyに投稿しました: {response.uri}")
