from apscheduler.schedulers.background import BackgroundScheduler
from apscheduler.jobstores.sqlalchemy import SQLAlchemyJobStore
from datetime import datetime, timedelta
from typing import List
import os

from src.web.scheduled_post_store import ScheduledPostStore
from src.web.post_executor import PostExecutor
from src.web.scheduled_post_model import ScheduledPost

def _monitor_scheduled_posts_job(scheduled_post_store: ScheduledPostStore, post_executor: PostExecutor):
    """
    予約投稿を監視し、実行日時が来たものをPostExecutorに渡します。
    """
    print("Monitoring scheduled posts...")
    now = datetime.now()
    posts = scheduled_post_store.get_all_posts()
    
    for post in posts:
        if post.status == "予約済み" and post.scheduled_at <= now:
            print(f"Found scheduled post to execute: {post.id}")
            # PostExecutorに実行を依頼
            # 実際のPostExecutorの呼び出しは非同期にするか、別のスレッドで行うべきだが、
            # 現時点ではシンプルに同期呼び出しとする
            post_executor.execute_post(post.id)

class SchedulerService:
    def __init__(self, scheduled_post_store: ScheduledPostStore, post_executor: PostExecutor, data_dir: str = "data"):
        self.scheduled_post_store = scheduled_post_store
        self.post_executor = post_executor
        
        jobstores = {
            'default': SQLAlchemyJobStore(url=f'sqlite:///{data_dir}/jobs.sqlite')
        }
        self.scheduler = BackgroundScheduler(jobstores=jobstores)

    def start(self):
        """
        スケジューラーを開始します。
        """
        if not self.scheduler.running:
            self.scheduler.start()
            self.scheduler.add_job(
                _monitor_scheduled_posts_job, 
                'interval', 
                minutes=1, 
                id='monitor_scheduled_posts', 
                args=[self.scheduled_post_store, self.post_executor]
            )
            print("Scheduler started and monitoring job added.")

    def shutdown(self):
        """
        スケジューラーを停止します。
        """
        if self.scheduler.running:
            self.scheduler.shutdown()
            print("Scheduler shut down.")

    def add_scheduled_post_to_scheduler(self, post: ScheduledPost):
        """
        新しい予約投稿をAPSchedulerに登録します。
        """
        # APSchedulerのジョブは_monitor_scheduled_postsで処理されるため、
        # ここでは特に何もしない。ScheduledPostStoreに保存されることで監視対象となる。
        pass

    def remove_scheduled_post_from_scheduler(self, post_id: str):
        """
        APSchedulerから予約投稿を削除します。
        """
        # _monitor_scheduled_postsで処理されるため、ここでは特に何もしない。
        pass
