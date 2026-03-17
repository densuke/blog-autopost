from datetime import datetime, timedelta
from unittest.mock import ANY, MagicMock, patch
import pytest

from src.web.post_executor import PostExecutor
from src.web.scheduled_post_model import ScheduledPost
from src.web.scheduled_post_store import ScheduledPostStore
from src.web.timezone_utils import ensure_local_timezone
from src.config_manager import ConfigManager


@pytest.fixture
def mock_config_manager():
    """
    ConfigManagerのモックを作成します。
    """
    mock_config = MagicMock(spec=ConfigManager)
    mock_config.config = {
        "character_limits": {"x": 280, "bluesky": 300, "mastodon": 500, "misskey": 3000}
    }
    mock_config.get_allowed_timings_tolerance_minutes.return_value = 5
    return mock_config


@pytest.fixture
def mock_scheduled_post_store():
    """
    ScheduledPostStoreのモックを作成します。
    """
    return MagicMock(spec=ScheduledPostStore)


@pytest.fixture
def mock_core_posting_logic():
    """
    CorePostingLogicのモックを作成します。
    """
    return MagicMock()


@pytest.fixture
def mock_timing_validator():
    """TimingValidatorのモックを作成します。"""
    validator = MagicMock()
    validator.validate_timing.return_value = (True, None)
    return validator


@pytest.fixture
def post_executor(
    mock_scheduled_post_store,
    mock_config_manager,
    mock_core_posting_logic,
    mock_timing_validator,
):
    """
    PostExecutorのインスタンスを作成します。
    """
    executor = PostExecutor(
        mock_scheduled_post_store,
        mock_config_manager,
        mock_core_posting_logic,
        timing_validator=mock_timing_validator,
    )
    return executor


@pytest.fixture
def sample_scheduled_post():
    """
    テスト用のサンプルScheduledPostオブジェクトを作成します。
    """
    return ScheduledPost(
        scheduled_at=datetime.now() + timedelta(hours=1),
        content="Test post content",
        target_sns=["x", "bluesky"],
    )


def test_execute_post_success(
    mock_scheduled_post_store,
    mock_core_posting_logic,
    post_executor,
    sample_scheduled_post,
):
    """
    execute_postが成功した場合に、投稿ステータスが「実行済み」に更新されることを確認します。
    """
    mock_scheduled_post_store.get_post_by_id.return_value = sample_scheduled_post
    mock_core_posting_logic.post_to_sns.return_value = {
        "success": True,
        "results": {"x": "success", "bluesky": "success"},
        "errors": {},
    }

    result = post_executor.execute_post(sample_scheduled_post.id)

    assert result is True
    mock_scheduled_post_store.get_post_by_id.assert_called_once_with(
        sample_scheduled_post.id
    )
    mock_scheduled_post_store.update_post.assert_called_once()

    # update_postの引数を確認
    args, kwargs = mock_scheduled_post_store.update_post.call_args
    updated_post_id, updates = args
    assert updated_post_id == sample_scheduled_post.id
    assert updates["status"] == "実行済み"
    assert updates["error_message"] is None
    assert isinstance(updates["updated_at"], datetime)

    # TimingValidatorが両SNS分呼び出されること
    post_executor.timing_validator.validate_timing.assert_any_call("x", ANY)
    post_executor.timing_validator.validate_timing.assert_any_call("bluesky", ANY)


def test_execute_post_uses_scheduled_time_for_validation(
    mock_scheduled_post_store,
    mock_core_posting_logic,
    post_executor,
    mock_timing_validator,
    sample_scheduled_post,
):
    """Scheduled posts should validate against their scheduled time."""
    mock_scheduled_post_store.get_post_by_id.return_value = sample_scheduled_post
    mock_core_posting_logic.post_to_sns.return_value = {
        "success": True,
        "results": {"x": "success", "bluesky": "success"},
        "errors": {},
    }

    post_executor.execute_post(sample_scheduled_post.id)

    expected_time = ensure_local_timezone(sample_scheduled_post.scheduled_at)
    assert expected_time is not None
    for call in mock_timing_validator.validate_timing.call_args_list:
        assert call.args[1] == expected_time


def test_execute_post_post_not_found(mock_scheduled_post_store, post_executor):
    """
    指定されたIDの投稿が見つからない場合にFalseを返すことを確認します。
    """
    mock_scheduled_post_store.get_post_by_id.return_value = None

    result = post_executor.execute_post("non_existent_id")

    assert result is False
    mock_scheduled_post_store.get_post_by_id.assert_called_once_with(
        "non_existent_id"
    )
    mock_scheduled_post_store.update_post.assert_not_called()


def test_execute_post_failure(
    mock_scheduled_post_store,
    mock_core_posting_logic,
    post_executor,
    sample_scheduled_post,
):
    """
    execute_postが失敗した場合に、投稿ステータスが「失敗」に更新されることを確認します。
    """
    mock_scheduled_post_store.get_post_by_id.return_value = sample_scheduled_post
    mock_core_posting_logic.post_to_sns.return_value = {
        "success": False,
        "results": {},
        "errors": {"x": "Connection timeout", "bluesky": "Authentication failed"},
    }

    result = post_executor.execute_post(sample_scheduled_post.id)

    assert result is False
    mock_scheduled_post_store.get_post_by_id.assert_called_once_with(
        sample_scheduled_post.id
    )
    mock_scheduled_post_store.update_post.assert_called_once()

    # update_postの引数を確認
    args, kwargs = mock_scheduled_post_store.update_post.call_args
    updated_post_id, updates = args
    assert updated_post_id == sample_scheduled_post.id
    assert updates["status"] == "失敗"
    assert "Connection timeout" in updates["error_message"]
    assert "Authentication failed" in updates["error_message"]
    assert isinstance(updates["updated_at"], datetime)
    post_executor.timing_validator.validate_timing.assert_any_call("x", ANY)
    post_executor.timing_validator.validate_timing.assert_any_call("bluesky", ANY)


def test_execute_post_skip_due_to_timing(
    mock_scheduled_post_store,
    mock_core_posting_logic,
    post_executor,
    sample_scheduled_post,
):
    """タイミング制約により全SNSがスキップされるケース。"""
    sample_scheduled_post.target_sns = ["x"]
    mock_scheduled_post_store.get_post_by_id.return_value = sample_scheduled_post

    post_executor.timing_validator.validate_timing.return_value = (
        False,
        "投稿時刻が許可されたタイミングの範囲外です",
    )

    result = post_executor.execute_post(sample_scheduled_post.id)

    assert result is True
    mock_core_posting_logic.post_to_sns.assert_not_called()
    mock_scheduled_post_store.update_post.assert_called_once()

    args, _ = mock_scheduled_post_store.update_post.call_args
    updated_post_id, updates = args
    assert updated_post_id == sample_scheduled_post.id
    assert updates["status"] == "スキップ"
    assert "投稿時刻が許可されたタイミングの範囲外" in updates["error_message"]


def test_execute_post_partial_skip(
    mock_scheduled_post_store,
    mock_core_posting_logic,
    post_executor,
    sample_scheduled_post,
):
    """一部のSNSのみスキップされ、残りが実行されるケース。"""
    sample_scheduled_post.target_sns = ["x", "bluesky"]
    mock_scheduled_post_store.get_post_by_id.return_value = sample_scheduled_post

    post_executor.timing_validator.validate_timing.side_effect = [
        (False, "許可されたタイミング外"),
        (True, None),
    ]

    mock_core_posting_logic.post_to_sns.return_value = {
        "success": True,
        "results": {"bluesky": "success"},
        "errors": {},
    }

    result = post_executor.execute_post(sample_scheduled_post.id)

    assert result is True
    mock_core_posting_logic.post_to_sns.assert_called_once()
    called_kwargs = mock_core_posting_logic.post_to_sns.call_args.kwargs
    assert called_kwargs["target_sns"] == ["bluesky"]

    mock_scheduled_post_store.update_post.assert_called_once()
    args, _ = mock_scheduled_post_store.update_post.call_args
    _, updates = args
    assert updates["status"] == "実行済み"
    assert "x: 許可されたタイミング外" in updates["error_message"]
