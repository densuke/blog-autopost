from datetime import datetime, timedelta
from unittest.mock import MagicMock
import pytest

from src.web.scheduler_service import _monitor_scheduled_posts_job
from src.web.scheduled_post_model import ScheduledPost

@pytest.fixture
def mock_scheduled_post_store():
    """ScheduledPostStore のモック"""
    mock = MagicMock()
    return mock

@pytest.fixture
def mock_post_executor():
    """PostExecutor のモック"""
    mock = MagicMock()
    return mock

@pytest.fixture
def sample_scheduled_post():
    """実行予定の投稿サンプル"""
    return ScheduledPost(
        scheduled_at=datetime.now() - timedelta(minutes=5),  # 過去の時刻に設定
        content="Test post content",
        target_sns=["x"]
    )

def test_monitor_scheduled_posts_executes_due_posts(mock_scheduled_post_store, mock_post_executor, sample_scheduled_post):
    """
    _monitor_scheduled_posts_jobが実行日時が来た予約投稿を正しく検出し、PostExecutorを呼び出すことを確認します。
    """
    # 実行日時が来た投稿をストアが返すようにモック
    mock_scheduled_post_store.get_all_posts.return_value = [sample_scheduled_post]
    
    _monitor_scheduled_posts_job(mock_scheduled_post_store, mock_post_executor, retention_hours=24)
    
    mock_scheduled_post_store.get_all_posts.assert_called_once()
    mock_post_executor.execute_post.assert_called_once_with(sample_scheduled_post.id)

def test_monitor_scheduled_posts_does_not_execute_future_posts(mock_scheduled_post_store, mock_post_executor):
    """
    _monitor_scheduled_posts_jobが未来の予約投稿を実行しないことを確認します。
    """
    future_post = ScheduledPost(
        scheduled_at=datetime.now() + timedelta(minutes=5), # 未来の時刻に設定
        content="Future post content",
        target_sns=["x"]
    )
    mock_scheduled_post_store.get_all_posts.return_value = [future_post]
    
    _monitor_scheduled_posts_job(mock_scheduled_post_store, mock_post_executor, retention_hours=24)
    
    mock_scheduled_post_store.get_all_posts.assert_called_once()
    mock_post_executor.execute_post.assert_not_called()

def test_monitor_scheduled_posts_does_not_execute_executed_posts(mock_scheduled_post_store, mock_post_executor):
    """
    _monitor_scheduled_posts_jobが既に実行済みの投稿を実行しないことを確認します。
    """
    executed_post = ScheduledPost(
        scheduled_at=datetime.now() - timedelta(minutes=10),
        content="Executed post content",
        target_sns=["x"],
        status="実行済み"
    )
    mock_scheduled_post_store.get_all_posts.return_value = [executed_post]
    
    _monitor_scheduled_posts_job(mock_scheduled_post_store, mock_post_executor, retention_hours=24)
    
    mock_scheduled_post_store.get_all_posts.assert_called_once()
    mock_post_executor.execute_post.assert_not_called()
