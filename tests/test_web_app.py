import pytest
from bs4 import BeautifulSoup
from unittest.mock import patch
from src.web.main_web import app

from bs4 import BeautifulSoup
from fastapi import FastAPI
from fastapi.testclient import TestClient
from unittest.mock import patch
from datetime import datetime, timedelta
from src.web.scheduled_post_model import ScheduledPost

# テスト用の認証情報
TEST_USERNAME = "testuser"
TEST_PASSWORD = "testpass"

@pytest.fixture
def client(tmp_path_factory):
    """テストクライアントを返すフィクスチャ"""
    with patch('src.web.auth_service.AuthService') as MockAuthService, \
         patch('src.web.main_web.config_manager') as MockConfigManager:

        # AuthServiceのインスタンス化をモック
        mock_auth_instance = MockAuthService.return_value
        def verify_creds(username, password):
            return username == TEST_USERNAME and password == TEST_PASSWORD
        mock_auth_instance.verify_credentials.side_effect = verify_creds

        # ConfigManagerのモック
        mock_config_manager_instance = MockConfigManager.return_value
        mock_config_manager_instance.get_secret_key.return_value = "test-secret"
        mock_config_manager_instance.get_all_sns_configs.return_value = [
            {'name': 'x-main', 'type': 'x'},
            {'name': 'bluesky-main', 'type': 'bluesky'}
        ]
        mock_config_manager_instance.get_all_sns_names.return_value = ['x-main', 'bluesky-main']

        # main_webのauth_serviceをモックインスタンスに置き換える
        from src.web import main_web
        main_web.auth_service = mock_auth_instance

        data_dir_for_test = tmp_path_factory.mktemp("data_test_app")
        with patch('src.web.main_web.DATA_DIR', new=str(data_dir_for_test)):
            with patch('src.web.main_web.scheduler_service') as MockSchedulerService:
                MockSchedulerService.scheduler.running = False
                MockSchedulerService.start.return_value = None
                with TestClient(app) as c:
                    yield c

@pytest.fixture
def logged_in_client(client):
    """ログイン済みのテストクライアントを返すフィクスチャ"""
    client.post(
        "/login",
        data={"username": TEST_USERNAME, "password": TEST_PASSWORD},
        follow_redirects=False
    )
    yield client
    client.get("/logout")

def test_fastapi_instance(client):
    assert isinstance(app, FastAPI)

def test_get_login_page(client):
    response = client.get("/login")
    assert response.status_code == 200

def test_login_success(client):
    response = client.post(
        "/login",
        data={"username": TEST_USERNAME, "password": TEST_PASSWORD},
        follow_redirects=False
    )
    assert response.status_code == 303
    assert response.headers["location"] == "/"

def test_login_failure(client):
    response = client.post(
        "/login",
        data={"username": TEST_USERNAME, "password": "wrongpassword"},
        follow_redirects=False
    )
    assert response.status_code == 401

def test_logout(logged_in_client):
    logout_response = logged_in_client.get("/logout", follow_redirects=False)
    assert logout_response.status_code == 303
    assert logout_response.headers["location"] == "/login"

def test_access_root_unauthenticated(client):
    response = client.get("/", follow_redirects=False)
    assert response.status_code == 303
    assert response.headers["location"] == "/login"

@patch('src.web.main_web.config_manager')
def test_get_main_page_authenticated(mock_config_manager, logged_in_client):
    mock_config_manager.get_all_sns_configs.return_value = [
        {'name': 'x-main', 'type': 'x'}
    ]
    response = logged_in_client.get("/")
    assert response.status_code == 200
    assert b'value="x-main"' in response.content

@patch('src.web.main_web.scheduled_post_store')
def test_get_main_page_with_scheduled_posts(mock_scheduled_post_store, logged_in_client):
    now = datetime.now()
    mock_posts = [ScheduledPost(id="p1", scheduled_at=now, content="c1"), ScheduledPost(id="p2", status="失敗", scheduled_at=now-timedelta(minutes=1), content="c2")]
    mock_scheduled_post_store.get_all_posts.return_value = mock_posts
    response = logged_in_client.get("/")
    assert response.status_code == 200
    soup = BeautifulSoup(response.content, 'html.parser')
    assert soup.find('tr', class_="warning") is not None

@patch('src.web.main_web.posting_service')
def test_post_api_endpoint(mock_posting_service, logged_in_client):
    mock_posting_service.post_now.return_value = {'x-main': {'success': True}}
    response = logged_in_client.post("/api/post", data={'text': 't', 'sns_targets': 'x-main'}, files={'media_files': ('f.jpg', b'', 'image/jpeg')})
    assert response.status_code == 200

@patch('src.web.main_web.scheduler_service.scheduler.add_job')
def test_schedule_api_endpoint(mock_add_job, logged_in_client):
    response = logged_in_client.post("/api/schedule", data={'text':'t', 'sns_targets':['x-main'], 'schedule_time':'2025-01-01T00:00'})
    assert response.status_code == 200
    mock_add_job.assert_called_once()

@patch('src.web.main_web.scheduled_post_store')
def test_delete_scheduled_post_from_ui(mock_scheduled_post_store, logged_in_client):
    mock_scheduled_post_store.get_post_by_id.return_value = ScheduledPost(status="予約済み", scheduled_at=datetime.now(), content="c")
    response = logged_in_client.delete(f"/api/scheduled-posts/some_id")
    assert response.status_code == 204

@patch('src.web.main_web.scheduled_post_store')
def test_re_execute_scheduled_post_from_ui(mock_scheduled_post_store, logged_in_client):
    post = ScheduledPost(status="失敗", scheduled_at=datetime.now(), content="c")
    mock_scheduled_post_store.get_post_by_id.return_value = post
    mock_scheduled_post_store.update_post.return_value = post
    response = logged_in_client.post(f"/api/scheduled-posts/some_id/re-execute")
    assert response.status_code == 200

@patch('src.web.main_web.post_executor')
@patch('src.web.main_web.scheduled_post_store')
def test_send_now_scheduled_post_from_ui(mock_store, mock_executor, logged_in_client):
    mock_store.get_post_by_id.return_value = ScheduledPost(status="予約済み", scheduled_at=datetime.now(), content="c")
    response = logged_in_client.post(f"/api/scheduled-posts/some_id/send-now")
    assert response.status_code == 200
    mock_executor.execute_post.assert_called_once()

@patch('src.web.main_web.scheduled_post_store')
def test_edit_scheduled_post_from_ui(mock_store, logged_in_client):
    post = ScheduledPost(status="予約済み", scheduled_at=datetime.now(), content="c")
    mock_store.get_post_by_id.return_value = post
    mock_store.update_post.return_value = post
    response = logged_in_client.put(f"/api/scheduled-posts/some_id", data={'content': 'new'})
    assert response.status_code == 200
    mock_store.update_post.assert_called_once()