from typing import Dict, Any
from src.web.scheduled_post_model import ScheduledPost
from src.web.scheduled_post_store import ScheduledPostStore
from src.web.core_posting_logic import CorePostingLogic
from src.config_manager import ConfigManager
from datetime import datetime

class PostExecutor:
    def __init__(self, scheduled_post_store: ScheduledPostStore, config_manager: ConfigManager):
        self.scheduled_post_store = scheduled_post_store
        self.core_posting_logic = CorePostingLogic(config_manager)

    def execute_post(self, post_id: str, debug: bool = False) -> bool:
        """
        指定された予約投稿を実行します。
        既存のmain.pyの投稿処理をラップし、投稿結果に基づいてScheduledPostStoreのステータスを更新します。
        
        Args:
            post_id: 予約投稿のID
            debug: デバッグモード
            
        Returns:
            bool: 投稿が成功した場合True
        """
        post = self.scheduled_post_store.get_post_by_id(post_id)
        if not post:
            print(f"Error: Scheduled post with ID {post_id} not found.")
            return False

        print(f"Executing post {post.id}: {post.content[:50]}...")

        # CorePostingLogicを使って投稿を実行
        result = self.core_posting_logic.post_to_sns(
            content=post.content,
            media_files=post.media_files if post.media_files else None,
            target_sns=post.target_sns if post.target_sns else None,
            optimize=False,  # 予約投稿では最適化は行わない（投稿作成時に行うべき）
            debug=debug
        )

        # 投稿結果に応じてステータスを更新
        if result['success']:
            updates = {"status": "実行済み", "updated_at": datetime.now(), "error_message": None}
            if debug:
                print(f"Post {post_id} executed successfully: {result['results']}")
        else:
            error_details = '; '.join([f"{k}: {v}" for k, v in result['errors'].items()])
            updates = {"status": "失敗", "error_message": error_details, "updated_at": datetime.now()}
            if debug:
                print(f"Post {post_id} failed: {error_details}")
        
        self.scheduled_post_store.update_post(post_id, updates)
        return result['success']
