import json
import os
from typing import Any, List, Optional

import requests

from ..image_resizer import create_image_resizer
from . import SocialMediaPlugin


class Tumblr(SocialMediaPlugin):
    """
    TumblrプラグインクラスでOAuth 2.0認証を使用してTumblrに投稿します
    """

    def __init__(self, client_id: str, client_secret: str, access_token: str, blog_name: str, config: Optional[dict] = None):
        """
        Tumblrプラグインを初期化します
        
        Args:
            client_id: Tumblr APIクライアントID
            client_secret: Tumblr APIクライアントシークレット  
            access_token: OAuth 2.0アクセストークン
            blog_name: 投稿先のブログ名（example.tumblr.comならexample）
            config: 追加設定
        """
        self.sns_type = "tumblr"
        self.client_id = client_id
        self.client_secret = client_secret
        self.access_token = access_token
        self.blog_name = blog_name
        self.config = config or {}

        # APIベースURL
        self.api_base = "https://api.tumblr.com/v2"

        # 認証ヘッダーの設定
        self.headers = {
            "Authorization": f"Bearer {self.access_token}",
            "Content-Type": "application/json"
        }

    def supports_rich_content(self) -> bool:
        """
        リッチコンテンツをサポートするかどうか
        
        Returns:
            bool: Tumblrはリンクカードなどをサポート
        """
        return True

    def post(self, optimized_text: str, media_files: Optional[List[str]] = None, **kwargs: Any) -> bool:
        """
        Tumblrに投稿します
        
        Args:
            optimized_text: 投稿テキスト
            media_files: 添付するメディアファイルのパスリスト（オプション）
            **kwargs: 追加パラメータ
            
        Returns:
            bool: 投稿成功時はTrue、失敗時はFalse
        """
        try:
            debug = kwargs.get('debug', False)
            dry_run = kwargs.get('dry_run', False)

            self._debug_print(f"Tumblr投稿開始: ブログ={self.blog_name}", debug)

            # 投稿内容を準備
            post_data = self._prepare_post_data(optimized_text, media_files, debug)

            if dry_run:
                self._debug_print("ドライラン: 実際の投稿はスキップされました", debug)
                print("[DRY RUN] Tumblr投稿内容:")
                print(f"  ブログ: {self.blog_name}")
                print(f"  投稿タイプ: {post_data.get('type', 'text')}")
                print(f"  内容: {optimized_text[:100]}...")
                if media_files:
                    print(f"  メディア: {len(media_files)}個のファイル")
                return True

            # 実際の投稿実行
            response = self._execute_post(post_data, debug)
            return self._handle_response(response, debug)

        except Exception as e:
            print(f"Tumblr投稿エラー: {e}")
            self._debug_print(f"エラー詳細: {str(e)}", debug)
            return False

    def _prepare_post_data(self, text: str, media_files: Optional[List[str]], debug: bool) -> dict:
        """
        投稿データを準備します
        
        Args:
            text: 投稿テキスト
            media_files: メディアファイルのリスト
            debug: デバッグモード
            
        Returns:
            dict: API送信用の投稿データ
        """
        # メディアファイルがある場合は写真投稿、ない場合はテキスト投稿
        if media_files and len(media_files) > 0:
            post_data: dict[str, Any] = {
                "type": "photo",
                "caption": text,
                "data": []
            }

            # 画像ファイルを処理
            for media_file in media_files:
                processed_file = self._process_media_file(media_file, debug)
                if processed_file:
                    post_data["data"].append(processed_file)

            self._debug_print(f"写真投稿として準備: {len(post_data['data'])}個の画像", debug)
        else:
            # テキスト投稿
            post_data = {
                "type": "text",
                "body": text
            }
            self._debug_print("テキスト投稿として準備", debug)

        # タグ設定があれば追加
        tags = self.config.get('tags', [])
        if tags:
            post_data["tags"] = ",".join(tags)
            self._debug_print(f"タグ設定: {tags}", debug)

        return post_data

    def _process_media_file(self, media_file: str, debug: bool) -> Optional[dict]:
        """
        メディアファイルを処理します
        
        Args:
            media_file: メディアファイルのパス
            debug: デバッグモード
            
        Returns:
            dict or None: 処理済みメディアデータ
        """
        try:
            if not os.path.exists(media_file):
                self._debug_print(f"メディアファイルが見つかりません: {media_file}", debug)
                return None

            # 画像リサイザーを使って画像を処理
            resizer = create_image_resizer(self.config.get('image_settings', {}))
            processed_path = resizer.resize_image_file(media_file)

            # ファイルをバイナリで読み込み
            with open(processed_path, 'rb') as f:
                file_data = f.read()

            self._debug_print(f"メディアファイル処理完了: {media_file}", debug)

            return {
                "type": "image/jpeg",  # JPEGとして送信
                "data": file_data
            }

        except Exception as e:
            self._debug_print(f"メディアファイル処理エラー: {e}", debug)
            return None

    def _execute_post(self, post_data: dict, debug: bool) -> requests.Response:
        """
        実際の投稿を実行します
        
        Args:
            post_data: 投稿データ
            debug: デバッグモード
            
        Returns:
            requests.Response: API応答
        """
        url = f"{self.api_base}/blog/{self.blog_name}.tumblr.com/post"

        self._debug_print(f"投稿実行: {url}", debug)

        # メディアファイルがある場合はmultipart/form-dataで送信
        if post_data.get("type") == "photo" and post_data.get("data"):
            return self._post_with_media(url, post_data, debug)
        else:
            # テキスト投稿はJSONで送信
            return requests.post(url, headers=self.headers, json=post_data, timeout=30)

    def _post_with_media(self, url: str, post_data: dict, debug: bool) -> requests.Response:
        """
        メディア付き投稿を実行します
        
        Args:
            url: 投稿先URL
            post_data: 投稿データ
            debug: デバッグモード
            
        Returns:
            requests.Response: API応答
        """
        files = []
        data = {
            "type": "photo",
            "caption": post_data.get("caption", "")
        }

        # タグがあれば追加
        if "tags" in post_data:
            data["tags"] = post_data["tags"]

        # メディアファイルを準備
        for i, media_item in enumerate(post_data["data"]):
            files.append((f"data[{i}]", ("image.jpg", media_item["data"], media_item["type"])))

        # メディア投稿用のヘッダー（Content-Typeを除去）
        headers = {"Authorization": f"Bearer {self.access_token}"}

        self._debug_print(f"メディア付き投稿実行: {len(files)}個のファイル", debug)

        return requests.post(url, headers=headers, data=data, files=files, timeout=30)

    def _handle_response(self, response: requests.Response, debug: bool) -> bool:
        """
        API応答を処理します
        
        Args:
            response: API応答
            debug: デバッグモード
            
        Returns:
            bool: 成功時はTrue
        """
        self._debug_print(f"応答ステータス: {response.status_code}", debug)

        if response.status_code == 201:  # Created
            try:
                response_data = response.json()
                post_id = response_data.get("response", {}).get("id")
                print(f"Tumblr投稿成功: {self.blog_name}.tumblr.com (ID: {post_id})")
                self._debug_print(f"投稿ID: {post_id}", debug)
                return True
            except json.JSONDecodeError:
                print("Tumblr投稿成功（投稿IDの取得に失敗）")
                return True
        else:
            error_msg = f"Tumblr投稿失敗: HTTP {response.status_code}"
            try:
                error_data = response.json()
                if "errors" in error_data:
                    error_details = ", ".join([error.get("detail", str(error)) for error in error_data["errors"]])
                    error_msg += f" - {error_details}"
            except json.JSONDecodeError:
                error_msg += f" - {response.text[:200]}"

            print(error_msg)
            self._debug_print(f"エラー詳細: {response.text}", debug)
            return False
