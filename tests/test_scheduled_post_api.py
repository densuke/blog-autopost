from fastapi.testclient import TestClient
from fastapi import status
from datetime import datetime, timedelta
import pytest
import json
from pathlib import Path

from src.web.main_web import app, scheduled_post_store, get_current_user
from src.web.scheduled_post_model import ScheduledPost

# TestClientのインスタンスを作成
client = TestClient(app)

# 認証をモックする
def override_get_current_user():
    return "test_user"

app.dependency_overrides[get_current_user] = override_get_current_user

@pytest.fixture(autouse=True)
def clear_scheduled_posts_file():
    """
    各テストの前にscheduled_posts.jsonファイルをクリアします。
    """
    scheduled_post_store.file_path.parent.mkdir(parents=True, exist_ok=True)
    with open(scheduled_post_store.file_path, 'w', encoding='utf-8') as f:
        json.dump([], f, ensure_ascii=False, indent=4)
    yield
    if scheduled_post_store.file_path.exists():
        scheduled_post_store.file_path.unlink()

@pytest.fixture
def sample_post_data():
    """
    テスト用のサンプル予約投稿データを作成します。
    """
    return {
        "scheduled_at": (datetime.now() + timedelta(days=1)).isoformat(), # 未来の時刻に設定
        "content": "Test Post from API",
        "media_files": [],
        "target_sns": ["x", "bluesky"]
    }

@pytest.fixture
def pre_existing_post():
    """
    事前に存在する予約投稿を作成し、ストアに保存します。
    """
    # 未来の時刻で予約済み投稿を作成
    future_time = datetime.now() + timedelta(days=1)
    post = ScheduledPost(
        scheduled_at=future_time,
        content="Pre-existing Test Post",
        target_sns=["x", "bluesky"],
        status="予約済み"
    )
    scheduled_post_store.create_post(post)
    return post

# --- GET /api/scheduled-posts ---

def test_get_all_scheduled_posts_empty():
    """
    予約投稿がない場合に空のリストを返すことを確認します。
    """
    response = client.get("/api/scheduled-posts")
    assert response.status_code == status.HTTP_200_OK
    assert response.json() == []

def test_get_all_scheduled_posts_with_data(pre_existing_post):
    """
    予約投稿がある場合にすべての投稿を返すことを確認します。
    """
    response = client.get("/api/scheduled-posts")
    assert response.status_code == status.HTTP_200_OK
    data = response.json()
    assert len(data) == 1
    assert data[0]["id"] == pre_existing_post.id
    assert data[0]["content"] == pre_existing_post.content

# --- GET /api/scheduled-posts/{post_id} ---

def test_get_scheduled_post_by_id(pre_existing_post):
    """
    特定のIDの予約投稿を返すことを確認します。
    """
    response = client.get(f"/api/scheduled-posts/{pre_existing_post.id}")
    assert response.status_code == status.HTTP_200_OK
    data = response.json()
    assert data["id"] == pre_existing_post.id
    assert data["content"] == pre_existing_post.content

def test_get_scheduled_post_not_found():
    """
    存在しないIDの場合に404を返すことを確認します。
    """
    response = client.get("/api/scheduled-posts/non_existent_id")
    assert response.status_code == status.HTTP_404_NOT_FOUND

# --- POST /api/scheduled-posts ---

def test_create_scheduled_post(sample_post_data):
    """
    新しい予約投稿を正常に作成することを確認します。
    """
    response = client.post(
        "/api/scheduled-posts",
        data={
            "scheduled_at": sample_post_data["scheduled_at"],
            "content": sample_post_data["content"],
            "target_sns": sample_post_data["target_sns"]
        }
    )
    assert response.status_code == status.HTTP_201_CREATED
    data = response.json()
    assert "id" in data
    assert data["content"] == sample_post_data["content"]
    assert scheduled_post_store.get_post_by_id(data["id"]) is not None

def test_create_scheduled_post_invalid_data(sample_post_data):
    """
    無効なデータで予約投稿を作成しようとした場合に422を返すことを確認します。
    """
    invalid_data = sample_post_data.copy()
    invalid_data["scheduled_at"] = "invalid-date"
    response = client.post(
        "/api/scheduled-posts",
        data={
            "scheduled_at": invalid_data["scheduled_at"],
            "content": invalid_data["content"],
            "target_sns": invalid_data["target_sns"]
        }
    )
    assert response.status_code == status.HTTP_422_UNPROCESSABLE_ENTITY

# --- PUT /api/scheduled-posts/{post_id} ---

def test_update_scheduled_post(pre_existing_post):
    """
    既存の予約投稿を正常に更新することを確認します。
    """
    updated_content = "Updated content from API"
    response = client.put(
        f"/api/scheduled-posts/{pre_existing_post.id}",
        data={
            "content": updated_content,
            "scheduled_at": (datetime.now() + timedelta(days=2)).isoformat(),
            "target_sns": ["mastodon"]
        }
    )
    assert response.status_code == status.HTTP_200_OK
    data = response.json()
    assert data["id"] == pre_existing_post.id
    assert data["content"] == updated_content
    assert scheduled_post_store.get_post_by_id(pre_existing_post.id).content == updated_content

def test_update_scheduled_post_not_found():
    """
    存在しないIDの予約投稿を更新しようとした場合に404を返すことを確認します。
    """
    response = client.put(
        "/api/scheduled-posts/non_existent_id",
        data={
            "content": "Updated content",
            "scheduled_at": (datetime.now() + timedelta(days=2)).isoformat(),
            "target_sns": json.dumps(["mastodon"])
        }
    )
    assert response.status_code == status.HTTP_404_NOT_FOUND

def test_update_scheduled_post_conflict_executed(pre_existing_post):
    """
    実行済みの予約投稿を更新しようとした場合に409を返すことを確認します。
    """
    pre_existing_post.status = "実行済み"
    scheduled_post_store.update_post(pre_existing_post.id, {"status": "実行済み"})
    response = client.put(
        f"/api/scheduled-posts/{pre_existing_post.id}",
        data={
            "content": "Updated content",
            "scheduled_at": (datetime.now() + timedelta(days=2)).isoformat(),
            "target_sns": json.dumps(["mastodon"])
        }
    )
    assert response.status_code == status.HTTP_409_CONFLICT

def test_update_scheduled_post_unsupported_sns(pre_existing_post, sample_post_data):
    """
    サポートされていないSNSを指定して予約投稿を更新しようとした場合に400を返すことを確認します。
    """
    invalid_data = sample_post_data.copy()
    invalid_data["target_sns"] = json.dumps(["unsupported_sns"])
    response = client.put(
        f"/api/scheduled-posts/{pre_existing_post.id}",
        data={
            "scheduled_at": invalid_data["scheduled_at"],
            "content": invalid_data["content"],
            "target_sns": invalid_data["target_sns"]
        }
    )
    assert response.status_code == status.HTTP_400_BAD_REQUEST
    assert "Unsupported SNS target" in response.json()["detail"]

# --- DELETE /api/scheduled-posts/{post_id} ---

def test_delete_scheduled_post(pre_existing_post):
    """
    既存の予約投稿を正常に削除することを確認します。
    """
    response = client.delete(f"/api/scheduled-posts/{pre_existing_post.id}")
    assert response.status_code == status.HTTP_204_NO_CONTENT
    assert scheduled_post_store.get_post_by_id(pre_existing_post.id) is None

def test_delete_scheduled_post_not_found():
    """
    存在しないIDの予約投稿を削除しようとした場合に404を返すことを確認します。
    """
    response = client.delete("/api/scheduled-posts/non_existent_id")
    assert response.status_code == status.HTTP_404_NOT_FOUND

def test_delete_scheduled_post_conflict_executed(pre_existing_post):
    """
    実行済みの予約投稿を削除しようとした場合に409を返すことを確認します。
    """
    pre_existing_post.status = "実行済み"
    scheduled_post_store.update_post(pre_existing_post.id, {"status": "実行済み"})
    response = client.delete(f"/api/scheduled-posts/{pre_existing_post.id}")
    assert response.status_code == status.HTTP_409_CONFLICT

# --- POST /api/scheduled-posts/{post_id}/re-execute ---

def test_re_execute_scheduled_post(pre_existing_post):
    """
    失敗した予約投稿を再実行できることを確認します。
    """
    pre_existing_post.status = "失敗"
    scheduled_post_store.update_post(pre_existing_post.id, {"status": "失敗"})
    response = client.post(f"/api/scheduled-posts/{pre_existing_post.id}/re-execute")
    assert response.status_code == status.HTTP_200_OK
    data = response.json()
    assert data["id"] == pre_existing_post.id
    assert data["status"] == "予約済み"
    # scheduled_atが更新されたことを確認（元の時刻より新しいことを確認）
    assert datetime.fromisoformat(data["scheduled_at"]) > pre_existing_post.scheduled_at

def test_re_execute_scheduled_post_not_found():
    """
    存在しないIDの予約投稿を再実行しようとした場合に404を返すことを確認します。
    """
    response = client.post("/api/scheduled-posts/non_existent_id/re-execute")
    assert response.status_code == status.HTTP_404_NOT_FOUND

def test_re_execute_scheduled_post_conflict_successful(pre_existing_post):
    """
    成功済みの予約投稿を再実行しようとした場合に409を返すことを確認します。
    """
    pre_existing_post.status = "実行済み"
    scheduled_post_store.update_post(pre_existing_post.id, {"status": "実行済み"})
    response = client.post(f"/api/scheduled-posts/{pre_existing_post.id}/re-execute")
    assert response.status_code == status.HTTP_409_CONFLICT

# --- POST /api/scheduled-posts/{post_id}/send-now ---

def test_send_scheduled_post_now(pre_existing_post):
    """
    予約投稿を即時送信できることを確認します。
    """
    response = client.post(f"/api/scheduled-posts/{pre_existing_post.id}/send-now")
    assert response.status_code == status.HTTP_200_OK
    data = response.json()
    assert data["id"] == pre_existing_post.id
    assert data["status"] == "実行済み"

def test_send_scheduled_post_now_not_found():
    """
    存在しないIDの予約投稿を即時送信しようとした場合に404を返すことを確認します。
    """
    response = client.post("/api/scheduled-posts/non_existent_id/send-now")
    assert response.status_code == status.HTTP_404_NOT_FOUND

def test_send_scheduled_post_now_conflict_executed(pre_existing_post):
    """
    実行済みの予約投稿を即時送信しようとした場合に409を返すことを確認します。
    """
    pre_existing_post.status = "実行済み"
    scheduled_post_store.update_post(pre_existing_post.id, {"status": "実行済み"})
    response = client.post(f"/api/scheduled-posts/{pre_existing_post.id}/send-now")
    assert response.status_code == status.HTTP_409_CONFLICT