import pytest
from datetime import datetime, timedelta, timezone
from uuid import UUID
from src.web.scheduled_post_model import ScheduledPost

class TestScheduledPostModel:
    def test_scheduled_post_creation(self):
        now = datetime.now(timezone.utc)
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

    def test_scheduled_at_can_be_past_for_executed_or_failed_post(self):
        now = datetime.now(timezone.utc)
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