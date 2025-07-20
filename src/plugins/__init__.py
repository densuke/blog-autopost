from typing import List, Optional

class SocialMediaPlugin:
    def post(self, optimized_text: str, media_files: Optional[List[str]] = None):
        """
        SNSに投稿します
        
        Args:
            optimized_text: 投稿テキスト
            media_files: 添付するメディアファイルのパスリスト（オプション）
        """
        raise NotImplementedError
