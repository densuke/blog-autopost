import logging
import os
from datetime import timedelta

from apscheduler.jobstores.memory import MemoryJobStore
from apscheduler.schedulers.background import BackgroundScheduler

from src.web.post_executor import PostExecutor, ScheduledPostStoreType
from src.web.timezone_utils import ensure_local_timezone, now_local

# ロガーの設定
logger = logging.getLogger(__name__)

def _monitor_scheduled_posts_job(scheduled_post_store: ScheduledPostStoreType, post_executor: PostExecutor, retention_hours: float):
    """
    予約投稿を監視し、実行日時が来たものをPostExecutorに渡します。
    """
    logger.debug("--- Running scheduled post monitor job ---")
    now_local_dt = now_local()
    logger.debug(f"Current local time: {now_local_dt.isoformat()}")

    try:
        posts = scheduled_post_store.get_all_posts()
        logger.debug(f"Found {len(posts)} posts to check.")

        executed_count = 0
        for post in posts:
            scheduled_time = ensure_local_timezone(post.scheduled_at)

            logger.debug(f"Checking post ID: {post.id}, Status: {post.status}, Scheduled: {scheduled_time.isoformat() if scheduled_time else 'N/A'}")
            # 投稿が予約済み状態で、スケジュール時刻を過ぎているかチェック
            if post.status == "予約済み" and scheduled_time and scheduled_time <= now_local_dt:
                logger.info(f"予約投稿を実行します: post_id={post.id}")
                success = post_executor.execute_post(post.id)
                if success:
                    executed_count += 1
                    logger.info(f"予約投稿の実行に成功しました: post_id={post.id}")
                else:
                    logger.error(f"予約投稿の実行に失敗しました: post_id={post.id}")

        if executed_count > 0:
            logger.info(f"スケジューラージョブ完了: {executed_count}件の投稿を実行しました。")

        cutoff = now_local_dt - timedelta(hours=retention_hours)
        removed = scheduled_post_store.delete_posts_older_than(cutoff, statuses=["実行済み"])
        if removed:
            logger.info(f"古い実行済み投稿を{removed}件削除しました（保持期間: {retention_hours}h）。")

    except Exception as e:
        logger.error(f"スケジューラージョブでエラーが発生しました: {e}", exc_info=True)
    logger.debug("--- Finished scheduled post monitor job ---")

class SchedulerService:
    def __init__(self, scheduled_post_store: ScheduledPostStoreType, post_executor: PostExecutor, data_dir: str = "data", completed_post_retention_hours: float = 12.0):
        self.scheduled_post_store = scheduled_post_store
        self.post_executor = post_executor
        self.completed_post_retention_hours = completed_post_retention_hours if completed_post_retention_hours > 0 else 12.0

        # データディレクトリを確保
        os.makedirs(data_dir, exist_ok=True)

        # メモリストアを使用（シリアライズ不可能な関数対応）
        jobstores = {
            'default': MemoryJobStore()
        }
        self.scheduler = BackgroundScheduler(jobstores=jobstores)

    def start(self):
        """
        スケジューラーを開始します。
        """
        if not self.scheduler.running:
            self.scheduler.start()
            # ジョブストアの代わりに、メモリストアを使用するか、
            # ジョブを追加する前に既存のジョブをクリア
            try:
                existing_job = self.scheduler.get_job('monitor_scheduled_posts')
                if existing_job:
                    self.scheduler.remove_job('monitor_scheduled_posts')
            except Exception:  # noqa: BLE001
                # ジョブが存在しない場合のエラーをキャッチ
                pass

            self.scheduler.add_job(
                _monitor_scheduled_posts_job,
                'interval',
                seconds=30,
                id='monitor_scheduled_posts',
                args=[self.scheduled_post_store, self.post_executor, self.completed_post_retention_hours],
                replace_existing=True
            )
            logger.info("スケジューラーを開始しました。")

    def shutdown(self):
        """
        スケジューラーを停止します。
        """
        if self.scheduler.running:
            self.scheduler.shutdown()
            logger.info("スケジューラーを停止しました。")

