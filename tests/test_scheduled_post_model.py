import pytest
from datetime import datetime, timedelta
from uuid import UUID
from src.web.scheduled_post_model import ScheduledPost

class TestScheduledPostModel:
    def test_scheduled_post_creation(self):
        now = datetime.now()
        future_time = now + timedelta(days=1)
        post_id = "a1b2c3d4-e5f6-7890-1234-567890abcdef"
        
        post = ScheduledPost(
            id=post_id,
            scheduled_at=future_time,
            content="テスト投稿",
            media_files=["/path/to/image.jpg"],
            target_sns=["x", "bluesky"],
            status="予約済み",
            error_message=None,
            created_at=now,
            updated_at=now
        )

        assert str(post.id) == post_id
        assert post.scheduled_at == future_time
        assert post.content == "テスト投稿"
        assert post.media_files == ["/path/to/image.jpg"]
        assert post.target_sns == ["x", "bluesky"]
        assert post.status == "予約済み"
        assert post.error_message is None
        assert post.created_at == now
        assert post.updated_at == now

    def test_scheduled_at_must_be_future_for_pending_post(self):
        now = datetime.now()
        past_time = now - timedelta(days=1)
        
        with pytest.raises(ValueError, match="scheduled_at must be in the future for '予約済み' status"):
            ScheduledPost(
                id="a1b2c3d4-e5f6-7890-1234-567890abcdef",
                scheduled_at=past_time,
                content="過去の予約投稿",
                media_files=[],
                target_sns=["x"],
                status="予約済み",
                error_message=None,
                created_at=now,
                updated_at=now
            )

    def test_scheduled_at_can_be_past_for_executed_or_failed_post(self):
        now = datetime.now()
        past_time = now - timedelta(days=1)
        
        # 実行済み
        post_executed = ScheduledPost(
            id="a1b2c3d4-e5f6-7890-1234-567890abcdef",
            scheduled_at=past_time,
            content="実行済み投稿",
            media_files=[],
            target_sns=["x"],
            status="実行済み",
            error_message=None,
            created_at=now,
            updated_at=now
        )
        assert post_executed.status == "実行済み"
        assert post_executed.scheduled_at == past_time

        # 失敗
        post_failed = ScheduledPost(
            id="a1b2c3d4-e5f6-7890-1234-567890abcdef",
            scheduled_at=past_time,
            content="失敗投稿",
            media_files=[],
            target_sns=["x"],
            status="失敗",
            error_message="エラーメッセージ",
            created_at=now,
            updated_at=now
        )
        assert post_failed.status == "失敗"
        assert post_failed.scheduled_at == past_time

    def test_status_restrictions_for_editing(self):
        now = datetime.now()
        future_time = now + timedelta(days=1)
        
        # 実行済み投稿は編集不可
        executed_post = ScheduledPost(
            id="a1b2c3d4-e5f6-7890-1234-567890abcdef",
            scheduled_at=now - timedelta(hours=1),
            content="実行済み",
            media_files=[],
            target_sns=["x"],
            status="実行済み",
            error_message=None,
            created_at=now - timedelta(days=2),
            updated_at=now - timedelta(days=2)
        )
        with pytest.raises(ValueError, match="Cannot edit a post with status '実行済み'"):
            executed_post.content = "新しい内容"

        # 失敗投稿は編集不可 (再実行は可能だが、直接編集は不可)
        failed_post = ScheduledPost(
            id="a1b2c3d4-e5f6-7890-1234-567890abcdef",
            scheduled_at=now - timedelta(hours=1),
            content="失敗",
            media_files=[],
            target_sns=["x"],
            status="失敗",
            error_message="エラー",
            created_at=now - timedelta(days=2),
            updated_at=now - timedelta(days=2)
        )
        with pytest.raises(ValueError, match="Cannot edit a post with status '失敗'"):
            failed_post.content = "新しい内容"
