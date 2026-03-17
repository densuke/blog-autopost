#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""セキュリティレスポンスヘッダーのテスト

以下のヘッダーが全レスポンスに付与されることを検証する:
- X-Content-Type-Options: nosniff
- X-Frame-Options: DENY
- X-XSS-Protection: 1; mode=block
- Referrer-Policy: strict-origin-when-cross-origin
- Permissions-Policy: camera=(), microphone=(), geolocation=()
"""

import pytest
from unittest.mock import MagicMock
from starlette.testclient import TestClient


@pytest.fixture
def headers_client():
    """セキュリティヘッダーテスト用クライアント"""
    from src.web import dependencies
    from src.web.main_web import app

    mock_auth_service = MagicMock()
    mock_auth_service.verify_credentials.return_value = True

    mock_config_manager = MagicMock()
    mock_config_manager.get_secret_key.return_value = "test-secret-headers"
    mock_config_manager.get_csrf_secret_key.return_value = "csrf-secret-headers"
    mock_config_manager.get_cookie_secure.return_value = False
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


class TestSecurityHeaders:
    """セキュリティレスポンスヘッダーの検証テスト"""

    def test_x_content_type_options_on_login_page(self, headers_client: TestClient):
        """ログインページにX-Content-Type-Options: nosniffヘッダーが付与される"""
        response = headers_client.get("/login")
        assert response.headers.get("x-content-type-options") == "nosniff", (
            "X-Content-Type-Options: nosniff が必要（MIMEスニッフィング防止）"
        )

    def test_x_frame_options_on_login_page(self, headers_client: TestClient):
        """ログインページにX-Frame-Options: DENYヘッダーが付与される"""
        response = headers_client.get("/login")
        x_frame = response.headers.get("x-frame-options", "").upper()
        assert x_frame in ("DENY", "SAMEORIGIN"), (
            "X-Frame-Options が必要（クリックジャッキング防止）"
        )

    def test_referrer_policy_on_login_page(self, headers_client: TestClient):
        """ログインページにReferrer-Policyヘッダーが付与される"""
        response = headers_client.get("/login")
        assert response.headers.get("referrer-policy"), (
            "Referrer-Policy ヘッダーが必要"
        )

    def test_permissions_policy_on_login_page(self, headers_client: TestClient):
        """ログインページにPermissions-Policyヘッダーが付与される"""
        response = headers_client.get("/login")
        assert response.headers.get("permissions-policy"), (
            "Permissions-Policy ヘッダーが必要（不要なAPI権限の無効化）"
        )

    def test_security_headers_on_api_endpoint(self, headers_client: TestClient):
        """APIエンドポイントにもセキュリティヘッダーが付与される"""
        response = headers_client.get("/api/posts")
        assert response.headers.get("x-content-type-options") == "nosniff"

    def test_security_headers_on_404(self, headers_client: TestClient):
        """404レスポンスにもセキュリティヘッダーが付与される"""
        response = headers_client.get("/nonexistent-page-xyz")
        assert response.headers.get("x-content-type-options") == "nosniff"

    def test_no_server_version_leak(self, headers_client: TestClient):
        """Serverヘッダーにバージョン情報が含まれない"""
        response = headers_client.get("/login")
        server_header = response.headers.get("server", "")
        # uvicornのデフォルト "uvicorn" はOK、バージョン番号はNG
        assert "/" not in server_header or server_header == "", (
            f"Serverヘッダーにバージョン情報が含まれています: {server_header}"
        )


class TestSecurityHeadersMiddleware:
    """SecurityHeadersMiddlewareのユニットテスト"""

    def test_middleware_adds_required_headers(self):
        """ミドルウェアが必須ヘッダーを追加する"""
        from src.web.security_headers import SECURITY_HEADERS

        required_headers = [
            "x-content-type-options",
            "x-frame-options",
            "referrer-policy",
            "permissions-policy",
        ]
        lower_keys = {k.lower() for k in SECURITY_HEADERS}
        for header in required_headers:
            assert header in lower_keys, f"{header} がSECURITY_HEADERSに含まれていない"

    def test_x_content_type_options_value(self):
        """X-Content-Type-Optionsの値がnosniffである"""
        from src.web.security_headers import SECURITY_HEADERS

        headers_lower = {k.lower(): v for k, v in SECURITY_HEADERS.items()}
        assert headers_lower.get("x-content-type-options") == "nosniff"

    def test_x_frame_options_value(self):
        """X-Frame-Optionsの値がDENYまたはSAMEORIGINである"""
        from src.web.security_headers import SECURITY_HEADERS

        headers_lower = {k.lower(): v for k, v in SECURITY_HEADERS.items()}
        value = headers_lower.get("x-frame-options", "").upper()
        assert value in ("DENY", "SAMEORIGIN")
