from atproto import Client, client_utils
from . import SocialMediaPlugin

class Bluesky(SocialMediaPlugin):
    def __init__(self, identifier, password):
        self.client = Client()
        # Blueskyにログイン
        self.client.login(identifier, password)

    def post(self, title: str, link: str):
        # TextBuilderを使用してリンクを含むテキストを作成
        text_with_link = client_utils.TextBuilder().text(f"{title} ").link(link, link)
        # Blueskyに投稿
        response = self.client.send_post(text=text_with_link)
        print(f"Blueskyに投稿しました: {response.uri}")
