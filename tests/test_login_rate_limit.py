#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""ログインエンドポイントのレート制限テスト"""

import pytest
from bs4 import BeautifulSoup
from unittest.mock import MagicMock
from starlette.testclient import TestClient


def _get_csrf_token(client: TestClient) -> str:
    """ログインページからCSRFトークンを取得するヘルパー"""
    login_page = client.get("/login")
    soup = BeautifulSoup(login_page.content, "html.parser")
    token_input = soup.find("input", {"name": "csrf_token"})
    return token_input.get("value") if token_input else ""


@pytest.fixture
def rate_limit_client():
    """レート制限テスト用クライアント（認証サービスをモック化済み）"""
    from src.web import dependencies
    from src.web.main_web import app

    mock_auth_service = MagicMock()
    mock_auth_service.verify_credentials.side_effect = (
        lambda u, p: u == "admin" and p == "correct"
    )

    mock_config_manager = MagicMock()
    mock_config_manager.get_secret_key.return_value = "test-secret-key-for-rate-limit"
    mock_config_manager.get_all_sns_configs.return_value = []
    mock_config_manager.get_all_sns_names.return_value = []

    mock_scheduler = MagicMock()

    overrides = {
        dependencies.get_auth_service: lambda: mock_auth_service,
        dependencies.get_config_manager: lambda: mock_config_manager,
        dependencies.get_scheduler_service: lambda: mock_scheduler,
    }
    app.dependency_overrides.update(overrides)

    with TestClient(app, raise_server_exceptions=False) as c:
        yield c

    for key in overrides:
        app.dependency_overrides.pop(key, None)


@pytest.fixture(autouse=True)
def reset_rate_limiter():
    """各テスト前後にレート制限の状態をリセットする"""
    from src.web.rate_limiter import limiter
    limiter.reset()
    yield
    limiter.reset()


class TestLoginRateLimit:
    """ログインエンドポイントのレート制限テスト"""

    def _post_login(self, client: TestClient, username: str = "admin", password: str = "wrong") -> object:
        """CSRFトークンを取得してログインPOSTを送るヘルパー"""
        csrf_token = _get_csrf_token(client)
        client.headers.update({"X-CSRFToken": csrf_token})
        return client.post(
            "/login",
            data={"username": username, "password": password, "csrf_token": csrf_token},
            follow_redirects=False,
        )

    def test_single_failed_login_returns_401(self, rate_limit_client):
        """1回の失敗ログインは401を返すことをテストする"""
        response = self._post_login(rate_limit_client, password="wrong")
        assert response.status_code == 401

    def test_successful_login_not_blocked(self, rate_limit_client):
        """正しい認証情報のログインはレート制限されないことをテストする"""
        response = self._post_login(rate_limit_client, password="correct")
        assert response.status_code in (200, 303)

    def test_too_many_login_attempts_return_429(self, rate_limit_client):
        """規定回数（5回/分）を超えるログイン試行は429を返すことをテストする"""
        for _ in range(5):
            self._post_login(rate_limit_client, password="wrong")

        # 6回目は 429 Too Many Requests になるはず
        response = self._post_login(rate_limit_client, password="wrong")
        assert response.status_code == 429, (
            f"6回目のリクエストは429になるはずが {response.status_code} が返った"
        )

    def test_rate_limit_resets_after_clear(self, rate_limit_client):
        """レート制限リセット後は再びリクエストを受け付けることをテストする"""
        from src.web.rate_limiter import limiter
        for _ in range(5):
            self._post_login(rate_limit_client, password="wrong")
        assert self._post_login(rate_limit_client, password="wrong").status_code == 429

        limiter.reset()
        response = self._post_login(rate_limit_client, password="wrong")
        assert response.status_code == 401
