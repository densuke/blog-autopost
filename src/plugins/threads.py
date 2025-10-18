import time
from typing import Any, List, Optional

import requests

from . import SocialMediaPlugin


class Threads(SocialMediaPlugin):
    def __init__(self, app_id: str, app_secret: str, access_token: str, config=None):
        """
        Threads API プラグインを初期化します
        
        Args:
            app_id: Meta App ID
            app_secret: Meta App Secret  
            access_token: 長期間有効なアクセストークン
            config: 追加設定
        """
        self.sns_type = "threads"
        self.app_id = app_id
        self.app_secret = app_secret
        self.access_token = access_token
        self.config = config or {}
        self.base_url = "https://graph.threads.net/v1.0"

        # ユーザーIDを取得
        self.user_id = self._get_user_id()
        if not self.user_id:
            raise ValueError("Threads APIへの認証に失敗しました。アクセストークンを確認してください。")

    def _get_user_id(self) -> Optional[str]:
        """
        現在のユーザーIDを取得します
        
        Returns:
            str: ユーザーID、失敗時はNone
        """
        url = f"{self.base_url}/me"
        headers = {"Authorization": f"Bearer {self.access_token}"}

        try:
            response = requests.get(url, headers=headers, timeout=10)
            response.raise_for_status()
            data = response.json()
            return data.get("id")
        except Exception as e:
            print(f"ユーザーID取得エラー: {e}")
            return None

    def _create_text_container(self, text: str, debug: bool = False) -> Optional[str]:
        """
        テキスト投稿用のコンテナを作成します
        
        Args:
            text: 投稿テキスト
            debug: デバッグモード
            
        Returns:
            str: 作成されたコンテナID、失敗時はNone
        """
        url = f"{self.base_url}/{self.user_id}/threads"
        headers = {"Authorization": f"Bearer {self.access_token}"}

        # テキストの文字数制限チェック（500文字）
        if len(text) > 500:
            if debug:
                print(f"[DEBUG] テキストが500文字を超えています: {len(text)}文字")
            text = text[:497] + "..."

        data = {
            "media_type": "TEXT",
            "text": text
        }

        try:
            if debug:
                print(f"[DEBUG] コンテナ作成リクエスト: {data}")

            response = requests.post(url, headers=headers, data=data, timeout=10)
            response.raise_for_status()
            result = response.json()

            container_id = result.get("id")
            if debug:
                print(f"[DEBUG] コンテナ作成成功: {container_id}")

            return container_id
        except Exception as e:
            print(f"コンテナ作成エラー: {e}")
            if debug and hasattr(e, 'response') and e.response:
                print(f"[DEBUG] レスポンス: {e.response.text}")
            return None

    def _create_image_container(self, image_url: str, text: str = "", debug: bool = False) -> Optional[str]:
        """
        画像投稿用のコンテナを作成します
        
        Args:
            image_url: 画像URL（アップロード済み）
            text: 投稿テキスト
            debug: デバッグモード
            
        Returns:
            str: 作成されたコンテナID、失敗時はNone
        """
        url = f"{self.base_url}/{self.user_id}/threads"
        headers = {"Authorization": f"Bearer {self.access_token}"}

        # テキストの文字数制限チェック（500文字）
        if len(text) > 500:
            if debug:
                print(f"[DEBUG] テキストが500文字を超えています: {len(text)}文字")
            text = text[:497] + "..."

        data = {
            "media_type": "IMAGE",
            "image_url": image_url,
            "text": text
        }

        try:
            if debug:
                print(f"[DEBUG] 画像コンテナ作成リクエスト: {data}")

            response = requests.post(url, headers=headers, data=data, timeout=10)
            response.raise_for_status()
            result = response.json()

            container_id = result.get("id")
            if debug:
                print(f"[DEBUG] 画像コンテナ作成成功: {container_id}")

            return container_id
        except Exception as e:
            print(f"画像コンテナ作成エラー: {e}")
            if debug and hasattr(e, 'response') and e.response:
                print(f"[DEBUG] レスポンス: {e.response.text}")
            return None

    def _publish_container(self, container_id: str, debug: bool = False) -> Optional[str]:
        """
        作成されたコンテナを公開します
        
        Args:
            container_id: コンテナID
            debug: デバッグモード
            
        Returns:
            str: 公開されたスレッドID、失敗時はNone
        """
        url = f"{self.base_url}/{self.user_id}/threads_publish"
        headers = {"Authorization": f"Bearer {self.access_token}"}

        data = {
            "creation_id": container_id
        }

        try:
            if debug:
                print(f"[DEBUG] コンテナ公開リクエスト: {data}")

            response = requests.post(url, headers=headers, data=data, timeout=10)
            response.raise_for_status()
            result = response.json()

            thread_id = result.get("id")
            if debug:
                print(f"[DEBUG] コンテナ公開成功: {thread_id}")

            return thread_id
        except Exception as e:
            print(f"コンテナ公開エラー: {e}")
            if debug and hasattr(e, 'response') and e.response:
                print(f"[DEBUG] レスポンス: {e.response.text}")
            return None

    def _debug_print(self, message: str, debug: bool = False) -> None:
        """デバッグメッセージを出力"""
        if debug:
            print(f"[DEBUG] {message}")

    def post(self, optimized_text: str, media_files: Optional[List[str]] = None, **kwargs: Any) -> Any:
        """
        Threadsに投稿します
        
        Args:
            optimized_text: 投稿テキスト
            media_files: 添付するメディアファイルのパスリスト（未実装）
            **kwargs: 追加パラメータ（debug: デバッグモードなど）
        """
        debug = kwargs.get('debug', False)

        try:
            # 現在はテキスト投稿のみサポート
            if media_files:
                print("⚠️  警告: Threads プラグインではメディア添付は現在サポートされていません")
                print("テキストのみで投稿します")

            self._debug_print(f"Threads投稿開始: {optimized_text[:50]}...", debug)

            # Step 1: コンテナ作成
            container_id = self._create_text_container(optimized_text, debug)
            if not container_id:
                raise Exception("コンテナの作成に失敗しました")

            # Step 2: 少し待機（APIの推奨）
            time.sleep(1)

            # Step 3: コンテナ公開
            thread_id = self._publish_container(container_id, debug)
            if not thread_id:
                raise Exception("コンテナの公開に失敗しました")

            print(f"Threadsに投稿しました: {thread_id}")

        except Exception as e:
            print(f"Threadsへの投稿中にエラー: {e}")
            raise

    def supports_rich_content(self) -> bool:
        """
        リッチコンテンツ（リンクカード等）をサポートするかどうか
        
        Returns:
            bool: False（現在は基本的なテキスト投稿のみ）
        """
        return False
