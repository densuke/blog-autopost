import requests
from . import SocialMediaPlugin

class Misskey(SocialMediaPlugin):
    def __init__(self, instance_url, access_token):
        self.instance_url = instance_url.rstrip('/')
        self.access_token = access_token
        self.api_url = f"{self.instance_url}/api"

    def post(self, title: str, link: str):
        # Misskeyに投稿するためのペイロード
        payload = {
            "i": self.access_token,
            "text": f"{title} {link}",
            "visibility": "public"
        }
        
        # APIエンドポイント
        url = f"{self.api_url}/notes/create"
        
        # POST リクエストを送信
        response = requests.post(url, json=payload)
        
        if response.status_code == 200:
            response_data = response.json()
            note_id = response_data.get('createdNote', {}).get('id')
            print(f"Misskeyに投稿しました: {self.instance_url}/notes/{note_id}")
        else:
            print(f"Misskeyへの投稿に失敗しました: {response.status_code}")
            print(f"エラー内容: {response.text}")
            raise Exception(f"Misskeyへの投稿に失敗しました: {response.status_code}")
