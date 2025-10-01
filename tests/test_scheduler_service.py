from datetime import datetime, timedelta
from unittest.mock import MagicMock, patch
import pytest
import os

from src.web.scheduler_service import SchedulerService
from src.web.scheduled_post_model import ScheduledPost
from src.web.scheduled_post_store import ScheduledPostStore
from src.web.post_executor import PostExecutor

@pytest.fixture
def mock_scheduled_post_store():
    """
    ScheduledPostStoreのモックを作成します。
    """
    return MagicMock(spec=ScheduledPostStore)

@pytest.fixture
def mock_post_executor():
    """
    PostExecutorのモックを作成します。
    """
    return MagicMock(spec=PostExecutor)

@pytest.fixture
def scheduler_service(mock_scheduled_post_store, mock_post_executor, tmp_path):
    """
    SchedulerServiceのインスタンスを作成します。
    """
    # APSchedulerがSQLiteファイルを作成するため、一時ディレクトリを使用
    data_dir = tmp_path / "scheduler_data"
    data_dir.mkdir()
    service = SchedulerService(mock_scheduled_post_store, mock_post_executor, str(data_dir))
    yield service
    # テスト後にスケジューラをシャットダウン
    if service.scheduler.running:
        service.scheduler.shutdown(wait=False)

@pytest.fixture
def sample_scheduled_post():
    """
    テスト用のサンプルScheduledPostオブジェクトを作成します。
    """
    return ScheduledPost(
        scheduled_at=datetime.now() - timedelta(minutes=5), # 過去の時刻に設定
        content="Test post content",
        target_sns=["x", "bluesky"]
    )

def test_monitor_scheduled_posts_executes_due_posts(scheduler_service, mock_scheduled_post_store, mock_post_executor, sample_scheduled_post):
    """
    _monitor_scheduled_postsが実行日時が来た予約投稿を正しく検出し、PostExecutorを呼び出すことを確認します。
    """
    # 実行日時が来た投稿をストアが返すようにモック
    mock_scheduled_post_store.get_all_posts.return_value = [sample_scheduled_post]
    
    scheduler_service._monitor_scheduled_posts()
    
    mock_scheduled_post_store.get_all_posts.assert_called_once()
    mock_post_executor.execute_post.assert_called_once_with(sample_scheduled_post.id)

def test_monitor_scheduled_posts_does_not_execute_future_posts(scheduler_service, mock_scheduled_post_store, mock_post_executor):
    """
    _monitor_scheduled_postsが未来の予約投稿を実行しないことを確認します。
    """
    future_post = ScheduledPost(
        scheduled_at=datetime.now() + timedelta(minutes=5), # 未来の時刻に設定
        content="Future post content",
        target_sns=["x"]
    )
    mock_scheduled_post_store.get_all_posts.return_value = [future_post]
    
    scheduler_service._monitor_scheduled_posts()
    
    mock_scheduled_post_store.get_all_posts.assert_called_once()
    mock_post_executor.execute_post.assert_not_called()

def test_monitor_scheduled_posts_does_not_execute_executed_posts(scheduler_service, mock_scheduled_post_store, mock_post_executor):
    """
    _monitor_scheduled_postsが既に実行済みの投稿を実行しないことを確認します。
    """
    executed_post = ScheduledPost(
        scheduled_at=datetime.now() - timedelta(minutes=10),
        content="Executed post content",
        target_sns=["x"],
        status="実行済み"
    )
    mock_scheduled_post_store.get_all_posts.return_value = [executed_post]
    
    scheduler_service._monitor_scheduled_posts()
    
    mock_scheduled_post_store.get_all_posts.assert_called_once()
    mock_post_executor.execute_post.assert_not_called()
