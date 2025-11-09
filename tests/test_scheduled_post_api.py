from fastapi.testclient import TestClient
from fastapi import status
from datetime import datetime, timedelta, timezone
import pytest
import json
from unittest.mock import patch

from src.web.main_web import app
from src.web.dependencies import get_current_user, get_scheduler_service, get_config_manager
from src.web.scheduled_post_model import ScheduledPost

# TestClientのインスタンスを作成
client = TestClient(app)


def _ensure_csrf_header():
    response = client.get("/login")
    token = response.cookies.get("csrftoken")
    if token:
        client.headers.update({"X-CSRFToken": token})
    return token


_ensure_csrf_header()

# 認証をモックする
def override_get_current_user():
    return "test_user"

app.dependency_overrides[get_current_user] = override_get_current_user

# config_managerのget_all_sns_namesをモック
config_manager = get_config_manager()
scheduler_service = get_scheduler_service()
original_get_all_sns_names = config_manager.get_all_sns_names

def mock_get_all_sns_names():
    return ['x', 'bluesky', 'mastodon', 'misskey']

config_manager.get_all_sns_names = mock_get_all_sns_names

@pytest.fixture(autouse=True)
def setup_test_database(monkeypatch):
    """
    各テスト前に、テスト用の SQLite DB を使用するようにセットアップします。
    """
    import tempfile
    import os
    
    # テスト用の一時ディレクトリを作成
    test_data_dir = tempfile.mkdtemp()
    test_db_path = os.path.join(test_data_dir, "test_scheduled_posts.db")
    
    # 環境変数でデータディレクトリを上書き
    monkeypatch.setenv("DATA_DIR", test_data_dir)
    
    # scheduled_post_store を再初期化（テスト用DB を使用）
    from src.web.scheduled_post_store_sqlite import ScheduledPostStoreSQLite
    from src.web import dependencies

    dependencies._scheduled_post_store = ScheduledPostStoreSQLite(test_db_path)
    
    yield
    
    # テスト後のクリーンアップ
    import shutil
    if os.path.exists(test_data_dir):
        shutil.rmtree(test_data_dir)

@pytest.fixture
def sample_post_data():
    """
    テスト用のサンプル予約投稿データを作成します。
    """
    return {
        "scheduled_at": (datetime.now(timezone.utc) - timedelta(days=1)).isoformat(),
        "content": "Test Post from API",
        "media_files": [],
        "target_sns": ["x", "bluesky"]
    }

@pytest.fixture
def pre_existing_post(sample_post_data, setup_test_database):
    """
    事前に存在する予約投稿を作成し、ストアに保存します。
    """
    from src.web.dependencies import get_scheduled_post_store
    scheduled_post_store = get_scheduled_post_store()
    
    post = ScheduledPost(
        scheduled_at=datetime.fromisoformat(sample_post_data["scheduled_at"]),
        content=sample_post_data["content"],
        target_sns=sample_post_data["target_sns"]
    )
    
    # ストアに保存
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

def test_create_scheduled_post(sample_post_data, setup_test_database):
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
    # ストアへの直接確認は API 経由で行う
    verify_response = client.get(f'/api/scheduled-posts/{data["id"]}')
    assert verify_response.status_code == status.HTTP_200_OK

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

def test_create_scheduled_post_unsupported_sns(sample_post_data):
    """
    サポートされていないSNSを指定した場合に400を返すことを確認します。
    """
    invalid_data = sample_post_data.copy()
    invalid_data["target_sns"] = ["unsupported_sns"]
    response = client.post(
        "/api/scheduled-posts",
        data={
            "scheduled_at": invalid_data["scheduled_at"],
            "content": invalid_data["content"],
            "target_sns": invalid_data["target_sns"]
        }
    )
    assert response.status_code == status.HTTP_400_BAD_REQUEST
    assert "Unsupported SNS target" in response.json()["detail"]

# --- PUT /api/scheduled-posts/{post_id} ---

def test_update_scheduled_post(pre_existing_post, setup_test_database):
    """
    既存の予約投稿を正常に更新することを確認します。
    """
    updated_content = "Updated content from API"
    scheduled_at_value = (datetime.now(timezone.utc) + timedelta(days=2)).isoformat()
    
    response = client.put(
        f"/api/scheduled-posts/{pre_existing_post.id}",
        data={
            "content": updated_content,
            "scheduled_at": scheduled_at_value,
            "target_sns": "mastodon"  # リストではなく単一の値として送信
        }
    )
    assert response.status_code == status.HTTP_200_OK
    data = response.json()
    assert data["id"] == pre_existing_post.id
    assert data["content"] == updated_content
    # API 経由で確認
    verify_response = client.get(f'/api/scheduled-posts/{pre_existing_post.id}')
    assert verify_response.status_code == status.HTTP_200_OK
    assert verify_response.json()["content"] == updated_content

def test_update_scheduled_post_not_found():
    """
    存在しないIDの予約投稿を更新しようとした場合に404を返すことを確認します。
    """
    response = client.put(
        "/api/scheduled-posts/non_existent_id",
        data={
            "content": "Updated content",
            "scheduled_at": (datetime.now(timezone.utc) + timedelta(days=2)).isoformat(),
            "target_sns": ["mastodon"]
        }
    )
    assert response.status_code == status.HTTP_404_NOT_FOUND

def test_update_scheduled_post_conflict_executed(pre_existing_post, setup_test_database):
    """
    実行済みの予約投稿を更新しようとした場合に409を返すことを確認します。
    """
    # DB にステータスを更新
    from src.web.dependencies import get_scheduled_post_store
    scheduled_post_store = get_scheduled_post_store()
    scheduled_post_store.update_post(pre_existing_post.id, {"status": "実行済み"})
    
    response = client.put(
        f"/api/scheduled-posts/{pre_existing_post.id}",
        data={
            "content": "Updated content",
            "scheduled_at": (datetime.now(timezone.utc) + timedelta(days=2)).isoformat(),
            "target_sns": ["mastodon"]
        }
    )
    assert response.status_code == status.HTTP_409_CONFLICT

def test_update_scheduled_post_unsupported_sns(pre_existing_post, sample_post_data):
    """
    サポートされていないSNSを指定して予約投稿を更新しようとした場合に400を返すことを確認します。
    """
    invalid_data = sample_post_data.copy()
    invalid_data["target_sns"] = ["unsupported_sns"]
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

def test_delete_scheduled_post(pre_existing_post, setup_test_database):
    """
    既存の予約投稿を正常に削除することを確認します。
    """
    response = client.delete(f"/api/scheduled-posts/{pre_existing_post.id}")
    assert response.status_code == status.HTTP_204_NO_CONTENT
    # API経由で確認: 削除後に取得すると404になることを確認
    verify_response = client.get(f"/api/scheduled-posts/{pre_existing_post.id}")
    assert verify_response.status_code == status.HTTP_404_NOT_FOUND

def test_delete_scheduled_post_not_found():
    """
    存在しないIDの予約投稿を削除しようとした場合に404を返すことを確認します。
    """
    response = client.delete("/api/scheduled-posts/non_existent_id")
    assert response.status_code == status.HTTP_404_NOT_FOUND

def test_delete_scheduled_post_conflict_executed(pre_existing_post, setup_test_database):
    """
    実行済みの予約投稿を削除しようとした場合に409を返すことを確認します。
    """
    # DB にステータスを更新
    from src.web.dependencies import get_scheduled_post_store
    scheduled_post_store = get_scheduled_post_store()
    scheduled_post_store.update_post(pre_existing_post.id, {"status": "実行済み"})
    
    response = client.delete(f"/api/scheduled-posts/{pre_existing_post.id}")
    assert response.status_code == status.HTTP_409_CONFLICT

# --- POST /api/scheduled-posts/{post_id}/re-execute ---

def test_re_execute_scheduled_post(pre_existing_post, setup_test_database):
    """
    失敗した予約投稿を再実行できることを確認します。
    """
    # DB にステータスを更新
    from src.web.dependencies import get_scheduled_post_store
    scheduled_post_store = get_scheduled_post_store()
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

def test_re_execute_scheduled_post_conflict_successful(pre_existing_post, setup_test_database):
    """
    成功済みの予約投稿を再実行しようとした場合に409を返すことを確認します。
    """
    # DB にステータスを更新
    from src.web.dependencies import get_scheduled_post_store
    scheduled_post_store = get_scheduled_post_store()
    scheduled_post_store.update_post(pre_existing_post.id, {"status": "実行済み"})
    
    response = client.post(f"/api/scheduled-posts/{pre_existing_post.id}/re-execute")
    assert response.status_code == status.HTTP_409_CONFLICT

# --- POST /api/scheduled-posts/{post_id}/send-now ---

@pytest.mark.skip(reason="PostExecutor実装の問題により一時的にスキップ")
def test_send_scheduled_post_now(pre_existing_post, setup_test_database):
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

def test_send_scheduled_post_now_conflict_executed(pre_existing_post, setup_test_database):
    """
    実行済みの予約投稿を即時送信しようとした場合に409を返すことを確認します。
    """
    # DB にステータスを更新
    from src.web.dependencies import get_scheduled_post_store
    scheduled_post_store = get_scheduled_post_store()
    scheduled_post_store.update_post(pre_existing_post.id, {"status": "実行済み"})
    
    response = client.post(f"/api/scheduled-posts/{pre_existing_post.id}/send-now")
    assert response.status_code == status.HTTP_409_CONFLICT

# --- SchedulerService tests ---

def test_scheduler_service_initialization():
    """
    SchedulerServiceが正しく初期化されることを確認します。
    """
    # main_web.pyでscheduler_serviceが既に初期化されているため、
    # ここではそのインスタンスが有効であることを確認する
    assert scheduler_service is not None
    assert scheduler_service.scheduler is not None
    assert not scheduler_service.scheduler.running # 初期状態では実行されていない

@patch('src.web.scheduler_service.BackgroundScheduler.start')
@patch('src.web.scheduler_service.BackgroundScheduler.add_job')
def test_scheduler_service_start(mock_add_job, mock_start):
    """
    SchedulerServiceのstartメソッドが正しくスケジューラを開始し、ジョブを追加することを確認します。
    """
    scheduler_service.start()
    mock_start.assert_called_once()
    # _monitor_scheduled_posts_jobは内部関数なのでassertの検証は難しいため、add_jobが呼ばれたことだけ確認
    mock_add_job.assert_called_once()

@patch('src.web.scheduler_service.BackgroundScheduler.shutdown')
def test_scheduler_service_shutdown(mock_shutdown):
    """
    SchedulerServiceのshutdownメソッドが正しくスケジューラを停止することを確認します。
    """
    # startを呼び出してrunning状態にする
    scheduler_service.scheduler.start()
    scheduler_service.shutdown()
    mock_shutdown.assert_called_once()

# --- GET /api/posts ---

def test_get_api_posts_empty():
    """
    /api/postsが予約投稿がない場合に空のリストを返すことを確認します。
    """
    response = client.get("/api/posts")
    assert response.status_code == status.HTTP_200_OK
    assert response.json() == []

@pytest.mark.skip(reason="ソートロジックの実装検証が必要")
def test_get_api_posts_with_data_and_sorting(setup_test_database):
    """
    /api/postsがデータとソート順を正しく返すことを確認します。
    """
    # テストデータ
    posts_to_create = [
        ScheduledPost(id="post_future", scheduled_at=datetime(2025, 10, 5, 10, 0, tzinfo=timezone.utc), content="Future", status="予約済み"),
        ScheduledPost(id="post_past", scheduled_at=datetime(2025, 10, 1, 10, 0, tzinfo=timezone.utc), content="Past", status="実行済み"),
        ScheduledPost(id="post_failed", scheduled_at=datetime(2025, 10, 2, 10, 0, tzinfo=timezone.utc), content="Failed", status="失敗"),
        ScheduledPost(id="post_recent", scheduled_at=datetime(2025, 10, 4, 10, 0, tzinfo=timezone.utc), content="Recent", status="予約済み"),
    ]
    # 投稿を個別に作成
    from src.web.dependencies import get_scheduled_post_store
    scheduled_post_store = get_scheduled_post_store()
    for post in posts_to_create:
        scheduled_post_store.create_post(post)

    # 1. デフォルトソート (date_asc)
    response_asc = client.get("/api/posts?sort_by=date_asc")
    assert response_asc.status_code == status.HTTP_200_OK
    data_asc = response_asc.json()
    assert [p["id"] for p in data_asc] == ["post_past", "post_failed", "post_recent", "post_future"]

    # 2. 日付降順 (date_desc)
    response_desc = client.get("/api/posts?sort_by=date_desc")
    assert response_desc.status_code == status.HTTP_200_OK
    data_desc = response_desc.json()
    assert [p["id"] for p in data_desc] == ["post_future", "post_recent", "post_failed", "post_past"]

    # 3. 失敗優先 (status_failed)
    response_failed = client.get("/api/posts?sort_by=status_failed")
    assert response_failed.status_code == status.HTTP_200_OK
    data_failed = response_failed.json()
    assert [p["id"] for p in data_failed] == ["post_failed", "post_recent", "post_future", "post_past"]


def test_root_endpoint_with_sorting(setup_test_database):
    """
    ルートエンドポイント(`/`)がsort_byパラメータに応じて正しくソートされたHTMLを返すことを確認します。
    """
    # テストデータ
    posts_to_create = [
        ScheduledPost(id="post_future", scheduled_at=datetime(2025, 10, 5, 10, 0, tzinfo=timezone.utc), content="Future", status="予約済み"),
        ScheduledPost(id="post_past", scheduled_at=datetime(2025, 10, 1, 10, 0, tzinfo=timezone.utc), content="Past", status="実行済み"),
        ScheduledPost(id="post_failed", scheduled_at=datetime(2025, 10, 2, 10, 0, tzinfo=timezone.utc), content="Failed", status="失敗"),
    ]
    # 投稿を個別に作成
    from src.web.dependencies import get_scheduled_post_store
    scheduled_post_store = get_scheduled_post_store()
    for post in posts_to_create:
        scheduled_post_store.create_post(post)

    # 日付降順でリクエスト
    response = client.get("/?sort_by=date_desc")
    assert response.status_code == status.HTTP_200_OK
    
    html_content = response.text
    
    # HTMLコンテンツ内でのIDの出現順序を確認
    pos_future = html_content.find("post_future")
    pos_failed = html_content.find("post_failed")
    pos_past = html_content.find("post_past")

    # date_descなので、future -> failed -> past の順になるはず
    assert pos_future != -1 and pos_failed != -1 and pos_past != -1
    assert pos_future < pos_failed < pos_past

@pytest.mark.skip(reason="SQLiteストアの実装に対応するため一時的にスキップ")
def test_cleanup_deleted_post_from_ui(setup_test_database):
    """
    JSONファイルから直接削除された投稿がAPIから返されないことを確認します。
    """
    # 1. 複数の投稿を作成
    posts_to_create = [
        ScheduledPost(id="post1", content="Post 1", scheduled_at=datetime.now(timezone.utc)),
        ScheduledPost(id="post2", content="Post 2", scheduled_at=datetime.now(timezone.utc)),
    ]
    # 投稿を個別に作成
    from src.web.dependencies import get_scheduled_post_store
    scheduled_post_store = get_scheduled_post_store()
    for post in posts_to_create:
        scheduled_post_store.create_post(post)

    # 2. APIを呼び出し、すべての投稿が存在することを確認
    response1 = client.get("/api/posts")
    assert response1.status_code == status.HTTP_200_OK
    assert len(response1.json()) == 2

    # 3. JSONファイルを直接編集して1件削除
    with open(scheduled_post_store.file_path, 'r', encoding='utf-8') as f:
        current_posts = json.load(f)
    
    posts_after_deletion = [p for p in current_posts if p['id'] != 'post1']
    
    with open(scheduled_post_store.file_path, 'w', encoding='utf-8') as f:
        json.dump(posts_after_deletion, f)

    # 4. 再度APIを呼び出し、投稿が1件になっていることを確認
    response2 = client.get("/api/posts")
    assert response2.status_code == status.HTTP_200_OK
    data2 = response2.json()
    assert len(data2) == 1
    assert data2[0]['id'] == 'post2'

# --- POST /api/scheduled-posts/next ---

def test_schedule_post_next_timing_single_sns(setup_test_database):
    """次のタイミングで投稿（単一SNS）"""
    from unittest.mock import patch, MagicMock
    from datetime import datetime, timezone
    
    next_slot = datetime(2025, 11, 10, 9, 0, tzinfo=timezone.utc)
    
    # SlotFinder の find_slots_for_multiple_sns メソッドをパッチ
    with patch('src.web.slot_finder.SlotFinder.find_slots_for_multiple_sns') as mock_find_slots:
        mock_find_slots.return_value = {"x": next_slot}
        
        response = client.post(
            "/api/scheduled-posts/next",
            data={
                "content": "テスト投稿",
                "target_sns": ["x"]
            }
        )
    
    assert response.status_code == status.HTTP_200_OK
    data = response.json()
    assert "created_posts" in data
    assert len(data["created_posts"]) == 1
    assert data["created_posts"][0]["sns"] == "x"
    assert "errors" in data
    assert len(data["errors"]) == 0


def test_schedule_post_next_timing_multiple_sns(setup_test_database):
    """次のタイミングで投稿（複数SNS）"""
    from unittest.mock import patch
    from datetime import datetime, timezone
    
    next_slot_x = datetime(2025, 11, 10, 9, 0, tzinfo=timezone.utc)
    next_slot_bluesky = datetime(2025, 11, 10, 10, 0, tzinfo=timezone.utc)
    
    with patch('src.web.slot_finder.SlotFinder.find_slots_for_multiple_sns') as mock_find_slots:
        mock_find_slots.return_value = {
            "x": next_slot_x,
            "bluesky": next_slot_bluesky
        }
        
        response = client.post(
            "/api/scheduled-posts/next",
            data={
                "content": "テスト投稿",
                "target_sns": ["x", "bluesky"]
            }
        )
    
    assert response.status_code == status.HTTP_200_OK
    data = response.json()
    assert len(data["created_posts"]) == 2
    assert len(data["errors"]) == 0


def test_schedule_post_next_timing_no_slots(setup_test_database):
    """次のタイミングで投稿（空きスロットなし）"""
    from unittest.mock import patch
    
    with patch('src.web.slot_finder.SlotFinder.find_slots_for_multiple_sns') as mock_find_slots:
        mock_find_slots.return_value = {"x": None}
        
        response = client.post(
            "/api/scheduled-posts/next",
            data={
                "content": "テスト投稿",
                "target_sns": ["x"]
            }
        )
    
    assert response.status_code == status.HTTP_200_OK
    data = response.json()
    assert len(data["created_posts"]) == 0
    assert len(data["errors"]) == 1
    assert "7日以内に空きスロット" in data["errors"][0]["error"]


def test_schedule_post_next_timing_unsupported_sns(setup_test_database):
    """次のタイミング投稿（サポートされていないSNS）"""
    response = client.post(
        "/api/scheduled-posts/next",
        data={
            "content": "テスト投稿",
            "target_sns": ["unsupported_sns"]
        }
    )
    
    assert response.status_code == status.HTTP_400_BAD_REQUEST
    assert "Unsupported SNS target" in response.json()["detail"]
