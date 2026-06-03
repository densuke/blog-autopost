import os
from typing import Any, Optional

import requests

from . import SocialMediaPlugin


class Mastodon(SocialMediaPlugin):
    def __init__(self, instance_url: str, access_token: str):
        self.sns_type = "mastodon"
        self.base_url = instance_url.rstrip('/')
        self.access_token = access_token

    def post(self, optimized_text: str, media_files: Optional[list] = None, **kwargs) -> Any:
        """
        Mastodonに投稿します
        
        Args:
            optimized_text: 投稿テキスト
            media_files: メディアファイルのパスリスト
        """
        # 投稿用ヘッダー（JSON）
        post_headers = {
            "Authorization": f"Bearer {self.access_token}",
            "User-Agent": "blog-autopost/1.0",
            "Content-Type": "application/json"
        }

        # メディアアップロード用ヘッダー（multipart/form-data）
        upload_headers = {
            "Authorization": f"Bearer {self.access_token}",
            "User-Agent": "blog-autopost/1.0"
        }

        media_ids = []

        # メディアファイルのアップロード
        if media_files:
            for media_path in media_files:
                try:
                    media_id = self._upload_media(media_path, upload_headers)
                    if media_id:
                        media_ids.append(media_id)
                        print(f"メディアアップロード完了: {media_path} (ID: {media_id})")
                except Exception as e:
                    print(f"メディアアップロードエラー: {media_path} - {e}")
                    # エラーが発生しても他のメディアの処理を続行

        # 投稿データを準備
        data: dict[str, Any] = {"status": optimized_text}
        if media_ids:
            # media_idsは配列として渡す
            data["media_ids"] = media_ids

        # 投稿実行
        url = f"{self.base_url}/api/v1/statuses"

        try:
            # JSONとして送信
            response = requests.post(url, headers=post_headers, json=data)

            if response.status_code in (200, 201):
                result = response.json()
                toot_id = result.get('id', 'unknown')
                toot_url = result.get('url', 'unknown')

                if media_ids:
                    print(f"Mastodonに投稿しました（メディア {len(media_ids)}件添付）: ID={toot_id}, URL={toot_url}")
                else:
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

    def _upload_media(self, media_path: str, headers: dict) -> Optional[str]:
        """
        メディアファイルをMastodonにアップロードします
        
        Args:
            media_path: メディアファイルのパス
            headers: リクエストヘッダー
            
        Returns:
            アップロードされたメディアのID、失敗時はNone
        """
        if not os.path.exists(media_path):
            print(f"メディアファイルが見つかりません: {media_path}")
            return None

        url = f"{self.base_url}/api/v2/media"

        try:
            with open(media_path, 'rb') as f:
                files = {'file': f}
                response = requests.post(url, headers=headers, files=files)

            if response.status_code in (200, 201, 202):
                result = response.json()
                media_id = result.get('id')

                # v2 APIの場合、処理が完了するまで待機が必要な場合がある
                if response.status_code == 202:
                    # 処理中の場合、少し待ってから状態をチェック
                    import time
                    time.sleep(1)
                    # 簡易的な実装：処理完了を待たずにIDを返す

                return media_id
            else:
                print(f"メディアアップロード失敗: {response.status_code}")
                try:
                    error_info = response.json()
                    print(f"エラー詳細: {error_info}")
                except ValueError:
                    print(f"エラーレスポンス: {response.text}")
                return None

        except requests.exceptions.RequestException as e:
            print(f"メディアアップロード通信エラー: {e}")
            return None
        except Exception as e:
            print(f"メディアアップロード中の予期しないエラー: {e}")
            return None
