import pytest
from bs4 import BeautifulSoup
from unittest.mock import patch

from bs4 import BeautifulSoup
from fastapi import FastAPI
from fastapi.testclient import TestClient
from unittest.mock import patch
from datetime import datetime, timedelta

# テスト用の認証情報をconfig_managerから取得
TEST_USERNAME = "testuser"
TEST_PASSWORD = "testpass"

@pytest.fixture(scope="session")
def logged_in_client(tmp_path_factory):
    """ログイン済みのテストクライアントを返すフィクスチャ"""
    # config_managerをモックしてテスト用の認証情報を返す
    with patch('src.web.main_web.config_manager') as MockConfigManager:
        mock_config_manager_instance = MockConfigManager.return_value
        mock_config_manager_instance.get_web_auth_credentials.return_value = {
            "username": TEST_USERNAME,
            "password": TEST_PASSWORD
        }

        # SchedulerServiceのdata_dirを一時ディレクトリに設定
        data_dir_for_test = tmp_path_factory.mktemp("data_test")
        with patch('src.web.main_web.DATA_DIR', new=str(data_dir_for_test)):
            with patch('src.web.main_web.scheduler_service') as MockSchedulerService:
                MockSchedulerService.scheduler.running = False # デフォルトでrunning=Falseにする
                MockSchedulerService.start.return_value = None # startメソッドが何もしないようにモック
                
                with TestClient(app) as client:
                    client.post(
                        "/login",
                        data={"username": TEST_USERNAME, "password": TEST_PASSWORD},
                        follow_redirects=False
                    )
                    yield client

def test_fastapi_instance(logged_in_client):
    """src.web.main_webにFastAPIインスタンスが存在することを確認する"""
    assert isinstance(app, FastAPI)

def test_get_login_page(logged_in_client):
    """/loginエンドポイントがログインフォームを返すことをテストする"""
    response = logged_in_client.get("/login")
    assert response.status_code == 200
    assert b'<form' in response.content
    assert b'name="username"' in response.content
    assert b'name="password"' in response.content

def test_login_success(logged_in_client): # logged_in_clientフィクスチャを使用
    """正しい認証情報でログインが成功し、リダイレクトとセッション設定が行われることをテストする"""
    # フィクスチャでログイン済みなので、ここではログイン後の状態をチェック
    response = logged_in_client.post(
        "/login",
        data={"username": TEST_USERNAME, "password": TEST_PASSWORD},
        follow_redirects=False
    )
    assert response.status_code == 303  # See Other, for redirect after POST
    assert response.headers["location"] == "/"
    assert "session" in response.cookies

def test_login_failure(logged_in_client):
    """間違った認証情報でログインが失敗することをテストする"""
    response = logged_in_client.post(
        "/login",
        data={"username": TEST_USERNAME, "password": "wrongpassword"}, # 間違ったパスワード
        follow_redirects=False
    )
    assert response.status_code == 401
    assert "session" not in response.cookies

def test_logout(logged_in_client):
    """ログアウト後、保護されたルートにアクセスするとリダイレクトされることをテストする"""
    # ログアウト
    logout_response = logged_in_client.get("/logout", follow_redirects=False)
    assert logout_response.status_code == 303
    assert logout_response.headers["location"] == "/login"

    # 保護されたルートにアクセスを試みる (未認証状態)
    response = logged_in_client.get("/", follow_redirects=False)
    assert response.status_code == 303
    assert response.headers["location"] == "/login"

def test_access_root_unauthenticated(logged_in_client):
    """未認証でルートにアクセスするとログインページにリダイレクトされることをテストする"""
    response = logged_in_client.get("/", follow_redirects=False)
    assert response.status_code == 303
    assert response.headers["location"] == "/login"

def test_get_main_page_authenticated(logged_in_client): # logged_in_clientフィクスチャを使用
    """認証済みユーザーがメインページにアクセスでき、投稿フォームが表示されることをテストする"""
    response = logged_in_client.get("/")
    assert response.status_code == 200
    assert b'<textarea' in response.content
    assert b'name="text"' in response.content
    assert b'value="x"' in response.content # config.ymlに設定されているSNSアカウントのチェックボックスが表示されることを確認

@patch('src.web.main_web.scheduled_post_store')
def test_get_main_page_with_scheduled_posts(mock_scheduled_post_store, logged_in_client):
    """
    認証済みユーザーがメインページにアクセスした際に、予約投稿の一覧が正しく表示されることをテストする。
    失敗した過去の投稿が強調表示されることも確認する。
    """
    now = datetime.now()
    mock_posts = [
        ScheduledPost(
            id="post1",
            scheduled_at=now + timedelta(hours=1),
            content="Future post",
            target_sns=["x"],
            status="予約済み"
        ),
        ScheduledPost(
            id="post2",
            scheduled_at=now - timedelta(hours=1),
            content="Failed past post",
            target_sns=["bluesky"],
            status="失敗",
            error_message="Some error"
        ),
        ScheduledPost(
            id="post3",
            scheduled_at=now - timedelta(hours=2),
            content="Successful past post",
            target_sns=["mastodon"],
            status="実行済み"
        ),
    ]
    mock_scheduled_post_store.get_all_posts.return_value = mock_posts

    response = logged_in_client.get("/")
    assert response.status_code == 200
    soup = BeautifulSoup(response.content, 'html.parser')

    # 各投稿のID、内容、ステータスが表示されていることを確認
    for post in mock_posts:
        assert soup.find(string=post.id) is not None
        assert soup.find(string=post.content) is not None
        assert soup.find(string=post.status) is not None

    # 失敗した過去の投稿が強調表示されていることを確認
    failed_row = soup.find('tr', class_="warning")
    assert failed_row is not None
    assert mock_posts[1].id in failed_row.text

    # 成功した過去の投稿は強調表示されていないことを確認
    successful_row = soup.find(string=mock_posts[2].id).find_parent('tr')
    assert "warning" not in successful_row.get('class', [])

def test_post_api_endpoint(logged_in_client): # logged_in_clientフィクスチャを使用
    """/api/postエンドポイントがPostingServiceを正しく呼び出すことをテストする"""
    with patch('src.web.main_web.posting_service') as mock_posting_service:
        mock_posting_service.post_now.return_value = {'x-main': {'success': True}}
        
        dummy_file_content = b"dummy image content"
        files = {'media_files': ('test.jpg', dummy_file_content, 'image/jpeg')}
        data = {
            'text': 'Test post',
            'url': 'http://example.com',
            'sns_targets': 'x-main'
        }

        response = logged_in_client.post("/api/post", data=data, files=files)

        assert response.status_code == 200
        assert response.json() == {'x-main': {'success': True}}

        mock_posting_service.post_now.assert_called_once()
        called_args, _ = mock_posting_service.post_now.call_args
        assert called_args[0]['text'] == 'Test post'
        assert called_args[0]['url'] == 'http://example.com'
        assert called_args[0]['sns_targets'] == ['x-main']
        assert len(called_args[0]['media_files']) == 1

def test_scheduler_lifecycle():
    """アプリケーションのライフサイクルでスケジューラが開始・停止されることをテストする"""
    with patch('src.web.main_web.scheduler_service') as mock_scheduler_service:
        with TestClient(app) as client:
            mock_scheduler_service.start.assert_called_once()
        mock_scheduler_service.shutdown.assert_called_once()

def test_schedule_api_endpoint(logged_in_client): # logged_in_clientフィクスチャを使用
    """/api/scheduleエンドポイントがスケジューラを正しく呼び出すことをテストする"""
    with patch('src.web.main_web.scheduler_service') as mock_scheduler_service:
        schedule_time = "2025-12-31T23:59:00"
        data = {
            'text': 'Scheduled post',
            'url': 'http://schedule.example.com',
            'sns_targets': ['x-main'],
            'schedule_time': schedule_time
        }

        response = logged_in_client.post("/api/schedule", data=data)

        assert response.status_code == 200
        response_json = response.json()
        assert response_json.get("message") == "Post scheduled successfully"
        assert "job_id" in response_json
        assert isinstance(response_json["job_id"], str) # job_idが文字列であることを確認

        # mock_scheduler_service.add_job.assert_called_once() # add_jobはSchedulerService内部で呼ばれるため、ここでは不要

@patch('src.web.main_web.scheduled_post_store')
def test_delete_scheduled_post_from_ui(mock_scheduled_post_store, logged_in_client):
    """
    Web UIからの予約投稿削除が正しく動作することを確認します。
    """
    mock_post = ScheduledPost(
        id="delete_post_id",
        scheduled_at=datetime.now() + timedelta(hours=1),
        content="Post to delete",
        target_sns=["x"],
        status="予約済み"
    )
    mock_scheduled_post_store.get_post_by_id.return_value = mock_post
    mock_scheduled_post_store.delete_post.return_value = "delete_post_id"

    response = logged_in_client.delete(f"/api/scheduled-posts/{mock_post.id}")
    assert response.status_code == 204
    mock_scheduled_post_store.delete_post.assert_called_once_with(mock_post.id)

@patch('src.web.main_web.scheduled_post_store')
def test_re_execute_scheduled_post_from_ui(mock_scheduled_post_store, logged_in_client):
    """
    Web UIからの失敗した予約投稿の再実行が正しく動作することを確認します。
    """
    now = datetime.now()
    mock_post = ScheduledPost(
        id="re_execute_post_id",
        scheduled_at=now - timedelta(hours=1),
        content="Failed post to re-execute",
        target_sns=["x"],
        status="失敗",
        error_message="Some error"
    )
    mock_scheduled_post_store.get_post_by_id.return_value = mock_post
    mock_scheduled_post_store.update_post.return_value = mock_post # update_postが呼ばれた後にモックオブジェクトを返す

    response = logged_in_client.post(f"/api/scheduled-posts/{mock_post.id}/re-execute")
    assert response.status_code == 200
    mock_scheduled_post_store.update_post.assert_called_once()
    # update_postの引数を確認
    args, kwargs = mock_scheduled_post_store.update_post.call_args
    updated_post_id, updates = args
    assert updated_post_id == mock_post.id
    assert updates["status"] == "予約済み"
    assert updates["error_message"] is None
    assert updates["scheduled_at"] > now # 更新されたscheduled_atが現在時刻より新しいことを確認

@patch('src.web.main_web.scheduled_post_store')
def test_send_now_scheduled_post_from_ui(mock_scheduled_post_store, logged_in_client):
    """
    Web UIからの予約投稿の即時送信が正しく動作することを確認します。
    """
    mock_post = ScheduledPost(
        id="send_now_post_id",
        scheduled_at=datetime.now() + timedelta(hours=1),
        content="Post to send now",
        target_sns=["x"],
        status="予約済み"
    )
    mock_scheduled_post_store.get_post_by_id.return_value = mock_post
    mock_scheduled_post_store.update_post.return_value = mock_post # update_postが呼ばれた後にモックオブジェクトを返す

    response = logged_in_client.post(f"/api/scheduled-posts/{mock_post.id}/send-now")
    assert response.status_code == 200
    mock_scheduled_post_store.update_post.assert_called_once()
    # update_postの引数を確認
    args, kwargs = mock_scheduled_post_store.update_post.call_args
    updated_post_id, updates = args
    assert updated_post_id == mock_post.id
    assert updates["status"] == "実行済み"