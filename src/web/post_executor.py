import logging
from typing import Dict, List, Optional, Union

from src.config_manager import ConfigManager
from src.timing_manager import TimingManager
from src.web.core_posting_logic import CorePostingLogic
from src.web.scheduled_post_store import ScheduledPostStore
from src.web.scheduled_post_store_sqlite import ScheduledPostStoreSQLite
from src.web.timing_validator import TimingValidator
from src.web.timezone_utils import now_local

# ロガーの設定
logger = logging.getLogger(__name__)

ScheduledPostStoreType = Union[ScheduledPostStore, ScheduledPostStoreSQLite]


class PostExecutor:
    def __init__(
        self,
        scheduled_post_store: ScheduledPostStoreType,
        config_manager: ConfigManager,
        core_posting_logic: Optional[CorePostingLogic] = None,
        timing_validator: Optional[TimingValidator] = None,
    ):
        self.scheduled_post_store = scheduled_post_store
        self.core_posting_logic = core_posting_logic or CorePostingLogic(config_manager)
        if timing_validator is None:
            tolerance = config_manager.get_allowed_timings_tolerance_minutes()
            timing_manager = TimingManager(config_manager)
            self.timing_validator = TimingValidator(timing_manager, tolerance)
        else:
            self.timing_validator = timing_validator
        self.config_manager = config_manager

    def execute_post(self, post_id: str, debug: bool = False) -> bool:
        """
        指定された予約投稿を実行します。

        投稿時刻が許容範囲内にあるかを検証し、範囲外の場合はスキップとして扱います。
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

        execution_time = now_local()
        scheduled_time = post.scheduled_at

        target_sns = post.target_sns or []
        allowed_sns: List[str] = []
        skipped_reasons: Dict[str, str] = {}

        if target_sns and self.timing_validator:
            for sns in target_sns:
                is_valid, reason = self.timing_validator.validate_timing(
                    sns, execution_time
                )
                if is_valid:
                    allowed_sns.append(sns)
                    continue

                skip_reason = (
                    reason
                    or "投稿時刻が許可されたタイミングの範囲外のためスキップしました"
                )
                skipped_reasons[sns] = skip_reason
                scheduled_str = scheduled_time.isoformat() if scheduled_time else "N/A"
                logger.info(
                    "Post %s skipped for SNS %s (scheduled: %s, execution: %s): %s",
                    post.id,
                    sns,
                    scheduled_str,
                    execution_time.isoformat(),
                    skip_reason,
                )
                if debug:
                    print(
                        f"Post {post.id} skipped for {sns}: {skip_reason} "
                        f"(scheduled: {scheduled_str}, execution: {execution_time.isoformat()})"
                    )
        else:
            allowed_sns = target_sns

        # 全SNSがスキップされた場合はステータスのみ更新
        if target_sns and not allowed_sns:
            error_message = (
                " ; ".join(
                    f"{sns}: {reason}" for sns, reason in skipped_reasons.items()
                )
                or "投稿時刻が許可されたタイミングの範囲外のためスキップしました"
            )

            updates = {
                "status": "スキップ",
                "error_message": error_message,
                "updated_at": execution_time,
            }
            self.scheduled_post_store.update_post(post_id, updates)
            logger.info(
                "Post %s marked as skipped for all targets. Reasons: %s",
                post.id,
                error_message,
            )
            if debug:
                print(f"Post {post.id} skipped: {error_message}")
            return True

        # CorePostingLogicを使って投稿を実行
        result = self.core_posting_logic.post_to_sns(
            content=post.content,
            media_files=post.media_files if post.media_files else None,
            target_sns=allowed_sns
            if allowed_sns
            else (post.target_sns if post.target_sns else None),
            optimize=False,  # 予約投稿では最適化は行わない（投稿作成時に行うべき）
            debug=debug,
        )

        # エラーメッセージの統合(スキップ理由 + 投稿失敗理由)
        error_messages: List[str] = []

        if skipped_reasons:
            error_messages.extend(
                f"{sns}: {reason}" for sns, reason in skipped_reasons.items()
            )

        if result.get("errors"):
            error_messages.extend(
                f"{sns}: {message}" for sns, message in result["errors"].items()
            )

        combined_error = " ; ".join(error_messages) if error_messages else None

        # 投稿結果に応じてステータスを更新
        if result["success"]:
            updates = {
                "status": "実行済み",
                "updated_at": execution_time,
                "error_message": combined_error,
            }
            logger.info(f"Post {post_id} executed successfully: {result['results']}")
            if debug:
                print(f"Post {post_id} executed successfully: {result['results']}")
        else:
            error_details = combined_error or "; ".join(
                f"{k}: {v}" for k, v in result.get("errors", {}).items()
            )
            updates = {
                "status": "失敗",
                "error_message": error_details,
                "updated_at": execution_time,
            }
            logger.error(f"Post {post_id} failed: {error_details}")
            if debug:
                print(f"Post {post_id} failed: {error_details}")

        self.scheduled_post_store.update_post(post_id, updates)
        return result["success"]
