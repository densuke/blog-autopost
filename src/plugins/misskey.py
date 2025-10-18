import mimetypes
import os
from typing import Any, Optional

import requests

from . import SocialMediaPlugin


class Misskey(SocialMediaPlugin):
    def __init__(self, instance_url, access_token, is_sensitive=False):
        self.sns_type = "misskey"
        self.instance_url = instance_url.rstrip('/')
        self.access_token = access_token
        self.is_sensitive = is_sensitive
        self.api_url = f"{self.instance_url}/api"

    def post(self, optimized_text: str, media_files: list[str] | None = None, **kwargs: Any) -> Any:
        """
        Misskeyに投稿します
        
        Args:
            optimized_text: 投稿テキスト
            media_files: メディアファイルのパスリスト
            **kwargs: 追加パラメータ（例：debug、article_data）
        """
        debug = kwargs.get('debug', False)
        file_ids = []

        # メディアファイルのアップロード
        if media_files:
            for media_path in media_files:
                try:
                    file_id = self._upload_file(media_path)
                    if file_id:
                        file_ids.append(file_id)
                        print(f"ファイルアップロード完了: {media_path} (ID: {file_id})")
                except Exception as e:
                    print(f"ファイルアップロードエラー: {media_path} - {e}")
                    # エラーが発生しても他のファイルの処理を続行

        # Misskeyに投稿するためのペイロード
        payload = {
            "i": self.access_token,
            "text": optimized_text,
            "visibility": "public"
        }

        # ファイルが添付されている場合は追加
        if file_ids:
            payload["fileIds"] = file_ids

        # APIエンドポイント
        url = f"{self.api_url}/notes/create"

        # POST リクエストを送信
        try:
            response = requests.post(url, json=payload)

            if response.status_code == 200:
                response_data = response.json()
                note_id = response_data.get('createdNote', {}).get('id')

                if file_ids:
                    print(f"Misskeyに投稿しました（ファイル {len(file_ids)}件添付）: {self.instance_url}/notes/{note_id}")
                else:
                    print(f"Misskeyに投稿しました: {self.instance_url}/notes/{note_id}")
            else:
                print(f"Misskeyへの投稿に失敗しました: {response.status_code}")
                print(f"エラー内容: {response.text}")
                raise Exception(f"Misskeyへの投稿に失敗しました: {response.status_code}")

        except requests.exceptions.RequestException as e:
            print(f"Misskey API通信エラー: {e}")
            raise Exception(f"Misskey API通信エラー: {e}")
        except Exception as e:
            print(f"Misskey投稿中の予期しないエラー: {e}")
            raise e

    def _upload_file(self, file_path: str) -> Optional[str]:
        """
        ファイルをMisskeyにアップロードします
        
        Args:
            file_path: ファイルのパス
            
        Returns:
            アップロードされたファイルのID、失敗時はNone
        """
        if not os.path.exists(file_path):
            print(f"ファイルが見つかりません: {file_path}")
            return None

        url = f"{self.api_url}/drive/files/create"

        # MIMEタイプを取得
        mime_type, _ = mimetypes.guess_type(file_path)
        if mime_type is None:
            mime_type = 'application/octet-stream'

        try:
            with open(file_path, 'rb') as f:
                files = {
                    'file': (os.path.basename(file_path), f, mime_type)
                }
                data = {
                    'i': self.access_token,
                    'isSensitive': 'true' if self.is_sensitive else 'false'
                }

                response = requests.post(url, files=files, data=data)

            if response.status_code == 200:
                result = response.json()
                file_id = result.get('id')
                return file_id
            else:
                print(f"ファイルアップロード失敗: {response.status_code}")
                try:
                    error_info = response.json()
                    print(f"エラー詳細: {error_info}")
                except ValueError:
                    print(f"エラーレスポンス: {response.text}")
                return None

        except requests.exceptions.RequestException as e:
            print(f"ファイルアップロード通信エラー: {e}")
            return None
        except Exception as e:
            print(f"ファイルアップロード中の予期しないエラー: {e}")
            return None
