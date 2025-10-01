from datetime import datetime, timedelta
from unittest.mock import MagicMock, patch
import pytest

from src.web.post_executor import PostExecutor
from src.web.scheduled_post_model import ScheduledPost
from src.web.scheduled_post_store import ScheduledPostStore

@pytest.fixture
def mock_scheduled_post_store():
    """
    ScheduledPostStoreのモックを作成します。
    """
    return MagicMock(spec=ScheduledPostStore)

@pytest.fixture
def post_executor(mock_scheduled_post_store):
    """
    PostExecutorのインスタンスを作成します。
    """
    return PostExecutor(mock_scheduled_post_store)

@pytest.fixture
def sample_scheduled_post():
    """
    テスト用のサンプルScheduledPostオブジェクトを作成します。
    """
    return ScheduledPost(
        scheduled_at=datetime.now() + timedelta(hours=1),
        content="Test post content",
        target_sns=["x", "bluesky"]
    )

def test_execute_post_success(mock_scheduled_post_store, post_executor, sample_scheduled_post):
    """
    execute_postが成功した場合に、投稿ステータスが「実行済み」に更新されることを確認します。
    """
    mock_scheduled_post_store.get_post_by_id.return_value = sample_scheduled_post
    
    # 既存の投稿処理をモック
    with patch('src.web.post_executor.print') as mock_print:
        result = post_executor.execute_post(sample_scheduled_post.id)
        
        assert result is True
        mock_scheduled_post_store.get_post_by_id.assert_called_once_with(sample_scheduled_post.id)
        mock_scheduled_post_store.update_post.assert_called_once()
        
        # update_postの引数を確認
        args, kwargs = mock_scheduled_post_store.update_post.call_args
        updated_post_id, updates = args
        assert updated_post_id == sample_scheduled_post.id
        assert updates["status"] == "実行済み"
        assert isinstance(updates["updated_at"], datetime)

def test_execute_post_post_not_found(mock_scheduled_post_store, post_executor):
    """
    指定されたIDの投稿が見つからない場合にFalseを返すことを確認します。
    """
    mock_scheduled_post_store.get_post_by_id.return_value = None
    
    with patch('src.web.post_executor.print') as mock_print:
        result = post_executor.execute_post("non_existent_id")
        
        assert result is False
        mock_scheduled_post_store.get_post_by_id.assert_called_once_with("non_existent_id")
        mock_scheduled_post_store.update_post.assert_not_called()
        mock_print.assert_called_with("Error: Scheduled post with ID non_existent_id not found.")

# TODO: 既存の投稿処理が失敗した場合のテストケースを追加
