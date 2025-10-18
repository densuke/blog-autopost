import logging
from typing import Optional

from src.config_manager import ConfigManager
from src.web.core_posting_logic import CorePostingLogic
from src.web.scheduled_post_store import ScheduledPostStore
from src.web.timezone_utils import now_local

# ロガーの設定
logger = logging.getLogger(__name__)

class PostExecutor:
    def __init__(
        self,
        scheduled_post_store: ScheduledPostStore,
        config_manager: ConfigManager,
        core_posting_logic: Optional[CorePostingLogic] = None
    ):
        self.scheduled_post_store = scheduled_post_store
        self.core_posting_logic = core_posting_logic or CorePostingLogic(config_manager)

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
            error_msg = f"Scheduled post with ID {post_id} not found."
            logger.error(error_msg)
            print(f"Error: {error_msg}")
            return False

        logger.info(f"Executing post {post.id}: {post.content[:50]}...")
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
            updates = {"status": "実行済み", "updated_at": now_local(), "error_message": None}
            logger.info(f"Post {post_id} executed successfully: {result['results']}")
            if debug:
                print(f"Post {post_id} executed successfully: {result['results']}")
        else:
            error_details = '; '.join([f"{k}: {v}" for k, v in result['errors'].items()])
            updates = {"status": "失敗", "error_message": error_details, "updated_at": now_local()}
            logger.error(f"Post {post_id} failed: {error_details}")
            if debug:
                print(f"Post {post_id} failed: {error_details}")

        self.scheduled_post_store.update_post(post_id, updates)
        return result['success']
