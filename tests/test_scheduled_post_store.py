import json
from pathlib import Path
import pytest
from datetime import datetime, timedelta

from src.web.scheduled_post_store import ScheduledPostStore
from src.web.scheduled_post_model import ScheduledPost

@pytest.fixture
def temp_file(tmp_path):
    """
    テスト用のテンポラリファイルパスを生成し、テスト後に削除します。
    """
    file = tmp_path / "test_scheduled_posts.json"
    yield file
    if file.exists():
        file.unlink()

@pytest.fixture
def sample_posts():
    """
    テスト用のサンプル予約投稿リストを生成します。
    """
    return [
        ScheduledPost(
            id="post1",
            scheduled_at=datetime.now() + timedelta(days=1),
            content="Test Post 1",
            target_sns=["x"],
            status="予約済み"
        ),
        ScheduledPost(
            id="post2",
            scheduled_at=datetime.now() + timedelta(days=2),
            content="Test Post 2",
            target_sns=["bluesky"],
            status="予約済み"
        ),
        ScheduledPost(
            id="post3",
            scheduled_at=datetime.now() - timedelta(days=1),
            content="Failed Post",
            target_sns=["mastodon"],
            status="失敗",
            error_message="Connection error"
        ),
    ]

def test_initialize_file_creates_empty_json_if_not_exists(temp_file):
    """
    ファイルが存在しない場合に、空のJSON配列でファイルを初期化することを確認します。
    """
    store = ScheduledPostStore(temp_file)
    assert temp_file.exists()
    with open(temp_file, 'r', encoding='utf-8') as f:
        content = json.load(f)
    assert content == []

def test_initialize_file_does_not_overwrite_existing_file(temp_file, sample_posts):
    """
    ファイルが既に存在する場合、既存のファイルを上書きしないことを確認します。
    """
    initial_content_dicts = [p.to_dict() for p in sample_posts]
    with open(temp_file, 'w', encoding='utf-8') as f:
        json.dump(initial_content_dicts, f)

    store = ScheduledPostStore(temp_file)
    assert temp_file.exists()
    with open(temp_file, 'r', encoding='utf-8') as f:
        content = json.load(f)
    assert content == initial_content_dicts

def test_read_posts_reads_correct_content(temp_file, sample_posts):
    """
    _read_postsがファイルから正しい内容を読み込むことを確認します。
    """
    expected_content_dicts = [p.to_dict() for p in sample_posts]
    with open(temp_file, 'w', encoding='utf-8') as f:
        json.dump(expected_content_dicts, f)
    
    store = ScheduledPostStore(temp_file)
    actual_posts = store._read_posts()
    assert len(actual_posts) == len(sample_posts)
    for actual, expected in zip(actual_posts, sample_posts):
        assert actual.id == expected.id
        assert actual.content == expected.content

def test_write_posts_writes_correct_content(temp_file, sample_posts):
    """
    _write_postsが正しい内容をファイルに書き込むことを確認します。
    """
    content_to_write = sample_posts[:1] # Write only one post
    
    store = ScheduledPostStore(temp_file)
    store._write_posts(content_to_write)
    
    with open(temp_file, 'r', encoding='utf-8') as f:
        actual_content_dicts = json.load(f)
    assert len(actual_content_dicts) == 1
    assert actual_content_dicts[0]["id"] == content_to_write[0].id

# --- CRUD operation tests (will be RED initially) ---

def test_get_all_posts(temp_file, sample_posts):
    """
    get_all_postsがすべての予約投稿を正しく取得することを確認します。
    """
    store = ScheduledPostStore(temp_file)
    store._write_posts(sample_posts) # Setup initial data
    
    posts = store.get_all_posts()
    assert len(posts) == len(sample_posts)
    assert {p.id for p in posts} == {p.id for p in sample_posts}

def test_get_post_by_id(temp_file, sample_posts):
    """
    get_post_by_idが指定されたIDの予約投稿を正しく取得することを確認します。
    """
    store = ScheduledPostStore(temp_file)
    store._write_posts(sample_posts)

    post = store.get_post_by_id("post1")
    assert post is not None
    assert post.id == "post1"
    assert post.content == "Test Post 1"

    # 存在しないIDの場合
    non_existent_post = store.get_post_by_id("non_existent_id")
    assert non_existent_post is None

def test_create_post(temp_file):
    """
    create_postが新しい予約投稿を正しく作成することを確認します。
    """
    store = ScheduledPostStore(temp_file)
    new_post = ScheduledPost(
        scheduled_at=datetime.now() + timedelta(hours=1),
        content="New Post",
        target_sns=["threads"]
    )
    created_post = store.create_post(new_post)
    assert created_post.id == new_post.id
    assert created_post.content == new_post.content
    
    posts = store._read_posts()
    assert len(posts) == 1
    assert posts[0].id == new_post.id

def test_update_post(temp_file, sample_posts):
    """
    update_postが既存の予約投稿を正しく更新することを確認します。
    """
    store = ScheduledPostStore(temp_file)
    store._write_posts(sample_posts)

    updates = {"content": "Updated Content", "status": "実行済み"}
    updated_post = store.update_post("post1", updates)
    assert updated_post is not None
    assert updated_post.id == "post1"
    assert updated_post.content == "Updated Content"
    assert updated_post.status == "実行済み"

    posts = store._read_posts()
    post1_in_store = next((p for p in posts if p.id == "post1"), None)
    assert post1_in_store.content == "Updated Content"
    assert post1_in_store.status == "実行済み"

    # 存在しないIDの更新
    non_existent_update = store.update_post("non_existent_id", {"content": "No update"})
    assert non_existent_update is None

def test_delete_post(temp_file, sample_posts):
    """
    delete_postが指定されたIDの予約投稿を正しく削除することを確認します。
    """
    store = ScheduledPostStore(temp_file)
    store._write_posts(sample_posts)

    deleted_post_id = store.delete_post("post2")
    assert deleted_post_id == "post2"

    posts = store._read_posts()
    assert len(posts) == 2
    assert "post2" not in {p.id for p in posts}

    # 存在しないIDの削除
    non_existent_delete = store.delete_post("non_existent_id")
    assert non_existent_delete is None

def test_get_all_posts_with_sorting(temp_file):
    """
    get_all_postsがsort_byパラメータに従って正しく投稿をソートすることを確認します。
    """
    store = ScheduledPostStore(temp_file)
    
    # テストデータ
    posts_to_create = [
        ScheduledPost(id="post_future", scheduled_at=datetime(2025, 10, 5, 10, 0), content="Future", status="予約済み"),
        ScheduledPost(id="post_past", scheduled_at=datetime(2025, 10, 1, 10, 0), content="Past", status="実行済み"),
        ScheduledPost(id="post_failed", scheduled_at=datetime(2025, 10, 2, 10, 0), content="Failed", status="失敗"),
        ScheduledPost(id="post_recent", scheduled_at=datetime(2025, 10, 4, 10, 0), content="Recent", status="予約済み"),
    ]
    store._write_posts(posts_to_create)

    # 1. デフォルト (date_asc)
    sorted_posts_asc = store.get_all_posts(sort_by='date_asc')
    assert [p.id for p in sorted_posts_asc] == ["post_past", "post_failed", "post_recent", "post_future"]

    # 2. 日付降順 (date_desc)
    sorted_posts_desc = store.get_all_posts(sort_by='date_desc')
    assert [p.id for p in sorted_posts_desc] == ["post_future", "post_recent", "post_failed", "post_past"]

    # 3. 失敗優先 (status_failed)
    # 失敗 -> 予約済み -> 実行済みの順になることを期待
    sorted_posts_failed = store.get_all_posts(sort_by='status_failed')
    assert [p.id for p in sorted_posts_failed] == ["post_failed", "post_future", "post_recent", "post_past"] or \
           [p.id for p in sorted_posts_failed] == ["post_failed", "post_recent", "post_future", "post_past"] # 予約済み内の順序は問わない

    # 4. 完了優先 (status_completed)
    # 実行済み -> 予約済み -> 失敗の順になることを期待
    sorted_posts_completed = store.get_all_posts(sort_by='status_completed')
    assert [p.id for p in sorted_posts_completed] == ["post_past", "post_future", "post_recent", "post_failed"] or \
           [p.id for p in sorted_posts_completed] == ["post_past", "post_recent", "post_future", "post_failed"] # 予約済み内の順序は問わない
           
    # 5. 引数なし（デフォルトの動作確認）
    default_sorted_posts = store.get_all_posts()
    assert [p.id for p in default_sorted_posts] == ["post_past", "post_failed", "post_recent", "post_future"]