from typing import Any, List, Optional


class SocialMediaPlugin:
    def post(self, optimized_text: str, media_files: Optional[List[str]] = None, **kwargs: Any):
        """
        SNSに投稿します
        
        Args:
            optimized_text: 投稿テキスト
            media_files: 添付するメディアファイルのパスリスト（オプション）
            **kwargs: SNS固有の追加パラメータ（例：article_data）
        """
        raise NotImplementedError

    def supports_rich_content(self) -> bool:
        """
        リッチコンテンツ（リンクカードなど）をサポートするかどうか
        
        Returns:
            bool: サポートする場合はTrue
        """
        return False

    def _debug_print(self, message: str, debug: bool = False) -> None:
        """
        デバッグ情報を出力します
        
        Args:
            message: 出力するメッセージ
            debug: デバッグモードが有効かどうか
        """
        if debug:
            print(f"[DEBUG] {message}")
