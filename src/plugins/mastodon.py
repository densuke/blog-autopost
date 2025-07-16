import requests
from . import SocialMediaPlugin

class Mastodon(SocialMediaPlugin):
    def __init__(self, instance_url: str, access_token: str):
        self.base_url = instance_url.rstrip('/')
        self.access_token = access_token

    def post(self, title: str, link: str):
        status_text = f"{title} {link}"
        url = f"{self.base_url}/api/v1/statuses"
        headers = {"Authorization": f"Bearer {self.access_token}"}
        data = {"status": status_text}
        response = requests.post(url, headers=headers, json=data)
        if response.status_code == 200:
            print(f"Mastodonに投稿しました: {response.json().get('id')}")
        else:
            print(f"Mastodon投稿失敗: {response.status_code} {response.text}")
