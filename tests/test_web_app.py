import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient
from unittest.mock import patch, MagicMock

from src.web.main_web import app, config_manager # config_managerをインポート

# テスト用の認証情報をconfig_managerから取得
TEST_USERNAME = config_manager.get_web_auth_credentials().get("username")
TEST_PASSWORD = config_manager.get_web_auth_credentials().get("password")

@pytest.fixture(scope="function") # functionスコープに変更
def logged_in_client():
    """ログイン済みのテストクライアントを返すフィクスチャ"""
    with TestClient(app) as client:
        client.post(
            "/login",
            data={"username": TEST_USERNAME, "password": TEST_PASSWORD},
            follow_redirects=False
        )
        yield client

def test_fastapi_instance():
    """src.web.main_webにFastAPIインスタンスが存在することを確認する"""
    assert isinstance(app, FastAPI)

def test_get_login_page():
    """/loginエンドポイントがログインフォームを返すことをテストする"""
    client = TestClient(app)
    response = client.get("/login")
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

def test_login_failure():
    """間違った認証情報でログインが失敗することをテストする"""
    client = TestClient(app)
    response = client.post(
        "/login",
        data={"username": TEST_USERNAME, "password": "wrongpassword"}, # 間違ったパスワード
        follow_redirects=False
    )
    assert response.status_code == 401
    assert "session" not in response.cookies

def test_logout(logged_in_client): # logged_in_clientフィクスチャを使用
    """ログアウト後、保護されたルートにアクセスするとリダイレクトされることをテストする"""
    # ログアウト
    logout_response = logged_in_client.get("/logout", follow_redirects=False)
    assert logout_response.status_code == 303
    assert logout_response.headers["location"] == "/login"

    # 保護されたルートにアクセスを試みる (未認証状態)
    response = logged_in_client.get("/", follow_redirects=False)
    assert response.status_code == 303
    assert response.headers["location"] == "/login"

def test_access_root_unauthenticated():
    """未認証でルートにアクセスするとログインページにリダイレクトされることをテストする"""
    client = TestClient(app) # 新しいクライアントインスタンス
    response = client.get("/", follow_redirects=False)
    assert response.status_code == 303
    assert response.headers["location"] == "/login"

def test_get_main_page_authenticated(logged_in_client): # logged_in_clientフィクスチャを使用
    """認証済みユーザーがメインページにアクセスでき、投稿フォームが表示されることをテストする"""
    response = logged_in_client.get("/")
    assert response.status_code == 200
    assert b'<textarea' in response.content
    assert b'name="text"' in response.content
    assert b'value="x"' in response.content # config.ymlに設定されているSNSアカウントのチェックボックスが表示されることを確認

from unittest.mock import patch

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
    with patch('src.web.main_web.scheduler') as mock_scheduler:
        with TestClient(app) as client:
            mock_scheduler.start.assert_called_once()
        mock_scheduler.shutdown.assert_called_once()

def test_schedule_api_endpoint(logged_in_client): # logged_in_clientフィクスチャを使用
    """/api/scheduleエンドポイントがスケジューラを正しく呼び出すことをテストする"""
    with patch('src.web.main_web.scheduler') as mock_scheduler:
        schedule_time = "2025-12-31T23:59:00"
        data = {
            'text': 'Scheduled post',
            'url': 'http://schedule.example.com',
            'sns_targets': 'x-main',
            'schedule_time': schedule_time
        }

        response = logged_in_client.post("/api/schedule", data=data)

        assert response.status_code == 200
        response_json = response.json()
        assert response_json.get("message") == "Post scheduled successfully"
        assert "job_id" in response_json
        assert isinstance(response_json["job_id"], str) # job_idが文字列であることを確認

        mock_scheduler.add_job.assert_called_once()