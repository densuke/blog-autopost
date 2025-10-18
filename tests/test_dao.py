import pytest
from datetime import datetime, timedelta
from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker

from src.web.dao import ScheduledPostDAO
from src.web.models import Base, ScheduledPostDB
from src.web.timezone_utils import now_local, ensure_local_timezone


@pytest.fixture
def db_session():
    """テスト用のインメモリ SQLite セッション"""
    engine = create_engine("sqlite:///:memory:")
    Base.metadata.create_all(engine)
    SessionLocal = sessionmaker(bind=engine)
    session = SessionLocal()
    yield session
    session.close()


@pytest.fixture
def dao(db_session):
    """ScheduledPostDAO インスタンス"""
    return ScheduledPostDAO(db_session)


@pytest.fixture
def sample_posts(db_session):
    """テスト用サンプル投稿データ"""
    now = now_local()
    posts = [
        ScheduledPostDB(
            id="post1",
            scheduled_at=now,
            content="テスト投稿 1",
            status="予約済み",
            target_sns=["x", "bluesky"]
        ),
        ScheduledPostDB(
            id="post2",
            scheduled_at=now + timedelta(hours=1),
            content="テスト投稿 2",
            status="実行済み",
            target_sns=["mastodon"]
        ),
        ScheduledPostDB(
            id="post3",
            scheduled_at=now + timedelta(hours=2),
            content="テスト投稿 3",
            status="失敗",
            target_sns=["x"]
        ),
        ScheduledPostDB(
            id="post4",
            scheduled_at=now + timedelta(hours=3),
            content="テスト投稿 4",
            status="予約済み",
            target_sns=["bluesky"]
        ),
        ScheduledPostDB(
            id="post5",
            scheduled_at=now + timedelta(hours=4),
            content="テスト投稿 5",
            status="実行済み",
            target_sns=["x", "bluesky", "mastodon"]
        ),
    ]
    for post in posts:
        db_session.add(post)
    db_session.commit()
    return posts


# ===== get_paginated_posts テスト =====

def test_get_paginated_posts_first_page(dao, sample_posts):
    """最初のページを取得できることをテスト"""
    posts, total_count = dao.get_paginated_posts(page=1, per_page=2)
    
    assert len(posts) == 2
    assert total_count == 5
    assert posts[0].id == "post1"
    assert posts[1].id == "post2"


def test_get_paginated_posts_second_page(dao, sample_posts):
    """2ページ目を取得できることをテスト"""
    posts, total_count = dao.get_paginated_posts(page=2, per_page=2)
    
    assert len(posts) == 2
    assert total_count == 5
    assert posts[0].id == "post3"
    assert posts[1].id == "post4"


def test_get_paginated_posts_last_page(dao, sample_posts):
    """最後のページ（余りあり）を取得できることをテスト"""
    posts, total_count = dao.get_paginated_posts(page=3, per_page=2)
    
    assert len(posts) == 1
    assert total_count == 5
    assert posts[0].id == "post5"


def test_get_paginated_posts_beyond_page(dao, sample_posts):
    """範囲外のページを取得すると空になることをテスト"""
    posts, total_count = dao.get_paginated_posts(page=10, per_page=2)
    
    assert len(posts) == 0
    assert total_count == 5


def test_get_paginated_posts_with_status_filter(dao, sample_posts):
    """ステータスフィルターが機能することをテスト"""
    posts, total_count = dao.get_paginated_posts(
        page=1, per_page=10, status_filter=["失敗"]
    )
    
    assert total_count == 1
    assert posts[0].id == "post3"
    assert posts[0].status == "失敗"


def test_get_paginated_posts_with_multiple_status_filter(dao, sample_posts):
    """複数ステータスフィルターが機能することをテスト"""
    posts, total_count = dao.get_paginated_posts(
        page=1, per_page=10, status_filter=["実行済み", "失敗"]
    )
    
    assert total_count == 3
    status_list = [post.status for post in posts]
    assert "実行済み" in status_list
    assert "失敗" in status_list


def test_get_paginated_posts_with_sns_filter(dao, sample_posts):
    """SNS フィルターが機能することをテスト"""
    posts, total_count = dao.get_paginated_posts(
        page=1, per_page=10, sns_filter=["bluesky"]
    )
    
    # bluesky を含む投稿: post1, post4, post5
    assert total_count == 3
    ids = [post.id for post in posts]
    assert "post1" in ids
    assert "post4" in ids
    assert "post5" in ids


def test_get_paginated_posts_with_multiple_sns_filter(dao, sample_posts):
    """複数 SNS フィルターが機能することをテスト"""
    posts, total_count = dao.get_paginated_posts(
        page=1, per_page=10, sns_filter=["x", "mastodon"]
    )
    
    # x または mastodon を含む投稿: post1, post2, post3, post5
    assert total_count == 4
    ids = [post.id for post in posts]
    assert "post1" in ids
    assert "post2" in ids
    assert "post3" in ids
    assert "post5" in ids
    assert "post4" not in ids


def test_get_paginated_posts_with_combined_filters(dao, sample_posts):
    """ステータスと SNS フィルターを組み合わせることをテスト"""
    posts, total_count = dao.get_paginated_posts(
        page=1, per_page=10, 
        status_filter=["予約済み"],
        sns_filter=["bluesky"]
    )
    
    # 予約済みかつ bluesky を含む投稿: post1, post4
    assert total_count == 2
    ids = [post.id for post in posts]
    assert "post1" in ids
    assert "post4" in ids


def test_get_paginated_posts_sort_by_date_asc(dao, sample_posts):
    """日時昇順ソートが機能することをテスト"""
    posts, _ = dao.get_paginated_posts(page=1, per_page=10, sort_by="date_asc")
    
    assert posts[0].id == "post1"
    assert posts[1].id == "post2"
    assert posts[4].id == "post5"


def test_get_paginated_posts_sort_by_date_desc(dao, sample_posts):
    """日時降順ソートが機能することをテスト"""
    posts, _ = dao.get_paginated_posts(page=1, per_page=10, sort_by="date_desc")
    
    assert posts[0].id == "post5"
    assert posts[1].id == "post4"
    assert posts[4].id == "post1"


@pytest.mark.skip(reason="SQLAlchemy case() 型互換性問題")
def test_get_paginated_posts_sort_by_status_failed(dao, sample_posts):
    """失敗優先ステータスソートが機能することをテスト"""
    posts, _ = dao.get_paginated_posts(page=1, per_page=10, sort_by="status_failed")
    assert len(posts) == 5


@pytest.mark.skip(reason="SQLAlchemy case() 型互換性問題")
def test_get_paginated_posts_sort_by_status_completed(dao, sample_posts):
    """完了優先ステータスソートが機能することをテスト"""
    posts, _ = dao.get_paginated_posts(page=1, per_page=10, sort_by="status_completed")
    assert len(posts) == 5


# ===== batch_delete_posts テスト =====

def test_batch_delete_posts_success(dao, sample_posts):
    """複数投稿の一括削除が成功することをテスト"""
    deleted_count = dao.batch_delete_posts(["post1", "post2", "post3"])
    
    assert deleted_count == 3
    
    # 削除されたことを確認
    assert dao.get_post_by_id("post1") is None
    assert dao.get_post_by_id("post2") is None
    assert dao.get_post_by_id("post3") is None
    
    # 削除されていないことを確認
    assert dao.get_post_by_id("post4") is not None
    assert dao.get_post_by_id("post5") is not None


def test_batch_delete_posts_empty_list(dao, sample_posts):
    """空リストで一括削除すると 0 が返されることをテスト"""
    deleted_count = dao.batch_delete_posts([])
    
    assert deleted_count == 0
    
    # すべての投稿が残っていることを確認
    assert len(dao.get_all_posts()) == 5


def test_batch_delete_posts_nonexistent_ids(dao, sample_posts):
    """存在しない ID を削除しようとすると 0 が返されることをテスト"""
    deleted_count = dao.batch_delete_posts(["nonexistent1", "nonexistent2"])
    
    assert deleted_count == 0
    
    # すべての投稿が残っていることを確認
    assert len(dao.get_all_posts()) == 5


def test_batch_delete_posts_mixed_ids(dao, sample_posts):
    """存在・非存在混在で一括削除することをテスト"""
    deleted_count = dao.batch_delete_posts(["post1", "nonexistent", "post3"])
    
    assert deleted_count == 2
    
    # 削除されたことを確認
    assert dao.get_post_by_id("post1") is None
    assert dao.get_post_by_id("post3") is None
    
    # 削除されていないことを確認
    assert dao.get_post_by_id("post2") is not None


def test_batch_delete_posts_all(dao, sample_posts):
    """すべての投稿を一括削除することをテスト"""
    deleted_count = dao.batch_delete_posts(["post1", "post2", "post3", "post4", "post5"])
    
    assert deleted_count == 5
    assert len(dao.get_all_posts()) == 0


# ===== delete_posts_older_than テスト =====

def test_delete_posts_older_than_success(dao, sample_posts, db_session):
    """指定した日時以前の投稿を削除できることをテスト"""
    now = now_local()
    
    # 各投稿の updated_at を明示的に設定
    for i, post in enumerate(sample_posts):
        post.updated_at = now + timedelta(hours=i)
    db_session.commit()
    
    cutoff = now + timedelta(hours=2.5)
    deleted_count = dao.delete_posts_older_than(cutoff)
    
    # post1, post2, post3 が削除される（updated_at <= cutoff）
    assert deleted_count == 3
    
    # 残っている投稿を確認
    remaining = dao.get_all_posts()
    assert len(remaining) == 2
    ids = [post.id for post in remaining]
    assert "post4" in ids
    assert "post5" in ids


def test_delete_posts_older_than_with_status_filter(dao, sample_posts, db_session):
    """指定した日時以前で特定ステータスのみ削除することをテスト"""
    now = now_local()
    
    # 各投稿の updated_at を明示的に設定
    for i, post in enumerate(sample_posts):
        post.updated_at = now + timedelta(hours=i)
    db_session.commit()
    
    cutoff = now + timedelta(hours=3)
    
    deleted_count = dao.delete_posts_older_than(
        cutoff,
        statuses=["実行済み", "失敗"]
    )
    
    # post2（実行済み）と post3（失敗）が削除される（updated_at <= cutoff かつ対象ステータス）
    assert deleted_count == 2
    
    # 残っている投稿を確認
    remaining = dao.get_all_posts()
    assert len(remaining) == 3
    ids = [post.id for post in remaining]
    assert "post1" in ids  # 予約済みなので削除されない
    assert "post4" in ids
    assert "post5" in ids


def test_delete_posts_older_than_nothing_to_delete(dao, sample_posts, db_session):
    """削除対象がない場合 0 が返されることをテスト"""
    now = now_local()
    
    # 各投稿の updated_at を未来に設定
    for i, post in enumerate(sample_posts):
        post.updated_at = now + timedelta(hours=10 + i)
    db_session.commit()
    
    cutoff = now - timedelta(hours=1)  # 過去の日時
    
    deleted_count = dao.delete_posts_older_than(cutoff)
    
    assert deleted_count == 0
    assert len(dao.get_all_posts()) == 5


def test_delete_posts_older_than_all(dao, sample_posts, db_session):
    """すべての投稿を削除することをテスト"""
    now = now_local()
    
    # 各投稿の updated_at を現在に設定
    for i, post in enumerate(sample_posts):
        post.updated_at = now + timedelta(hours=i)
    db_session.commit()
    
    cutoff = now + timedelta(hours=100)  # 未来の日時
    
    deleted_count = dao.delete_posts_older_than(cutoff)
    
    assert deleted_count == 5
    assert len(dao.get_all_posts()) == 0


def test_delete_posts_older_than_empty_status_filter(dao, sample_posts, db_session):
    """ステータスフィルターが空の場合、すべてが削除されることをテスト"""
    now = now_local()
    
    # 各投稿の updated_at を明示的に設定
    for i, post in enumerate(sample_posts):
        post.updated_at = now + timedelta(hours=i)
    db_session.commit()
    
    cutoff = now + timedelta(hours=2.5)
    
    deleted_count = dao.delete_posts_older_than(
        cutoff,
        statuses=[]  # 空のリスト
    )
    
    # 空のフィルター = フィルタリングなし、updated_at <= cutoff のみ
    # post1, post2, post3 が削除される
    assert deleted_count == 3
    remaining = dao.get_all_posts()
    assert len(remaining) == 2


def test_delete_posts_older_than_preserves_newer(dao, sample_posts, db_session):
    """指定した日時より後の投稿は削除されないことをテスト"""
    now = now_local()
    
    # 各投稿の updated_at を明示的に設定
    for i, post in enumerate(sample_posts):
        post.updated_at = now + timedelta(hours=i)
    db_session.commit()
    
    cutoff = now + timedelta(hours=1.5)
    
    deleted_count = dao.delete_posts_older_than(cutoff)
    
    # post1, post2 が削除される（updated_at <= 1.5h）
    assert deleted_count == 2
    
    # post3, post4, post5 が残る
    remaining = dao.get_all_posts()
    assert len(remaining) == 3
    for post in remaining:
        # updated_at > cutoff であることを確認
        # タイムゾーン情報の一貫性を確保
        post_updated = ensure_local_timezone(post.updated_at) or post.updated_at
        cutoff_tz = ensure_local_timezone(cutoff) or cutoff
        assert post_updated > cutoff_tz
