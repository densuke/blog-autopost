import requests
from . import SocialMediaPlugin

class Mastodon(SocialMediaPlugin):
    def __init__(self, instance_url: str, access_token: str):
        self.base_url = instance_url.rstrip('/')
        self.access_token = access_token

    def post(self, title: str, link: str):
        status_text = f"{title} {link}"
        url = f"{self.base_url}/api/v1/statuses"
        headers = {
            "Authorization": f"Bearer {self.access_token}",
            "User-Agent": "blog-autopost/1.0"
        }
        data = {"status": status_text}
        
        try:
            response = requests.post(url, headers=headers, data=data)
            
            if response.status_code in (200, 201):
                result = response.json()
                toot_id = result.get('id', 'unknown')
                toot_url = result.get('url', 'unknown')
                print(f"Mastodonに投稿しました: ID={toot_id}, URL={toot_url}")
            else:
                print(f"Mastodon投稿失敗: {response.status_code}")
                try:
                    error_info = response.json()
                    print(f"エラー詳細: {error_info}")
                except ValueError:
                    print(f"エラーレスポンス: {response.text}")
                
        except requests.exceptions.RequestException as e:
            print(f"Mastodon API通信エラー: {e}")
        except Exception as e:
            print(f"Mastodon投稿中の予期しないエラー: {e}")
