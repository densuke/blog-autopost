#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""ファイルアップロードのサイズ・MIME型バリデーションテスト"""

import io
import pytest
from unittest.mock import MagicMock, patch
from starlette.testclient import TestClient


@pytest.fixture
def upload_client():
    """アップロードバリデーションテスト用クライアント"""
    from src.web import dependencies
    from src.web.main_web import app

    mock_auth_service = MagicMock()
    mock_auth_service.verify_credentials.return_value = True

    mock_config_manager = MagicMock()
    mock_config_manager.get_secret_key.return_value = "test-secret-upload"
    mock_config_manager.get_all_sns_configs.return_value = [
        {"name": "x-main", "type": "x"}
    ]
    mock_config_manager.get_all_sns_names.return_value = ["x-main"]

    mock_scheduler = MagicMock()
    mock_posting_service = MagicMock()
    mock_posting_service.post_now.return_value = {"results": []}
    mock_ticket_manager = MagicMock()
    mock_ticket_manager.create_ticket.return_value = "ticket-001"
    mock_store = MagicMock()
    mock_executor = MagicMock()

    overrides = {
        dependencies.get_auth_service: lambda: mock_auth_service,
        dependencies.get_config_manager: lambda: mock_config_manager,
        dependencies.get_scheduler_service: lambda: mock_scheduler,
        dependencies.get_posting_service: lambda: mock_posting_service,
        dependencies.get_ticket_manager: lambda: mock_ticket_manager,
        dependencies.get_scheduled_post_store: lambda: mock_store,
        dependencies.get_executor: lambda: mock_executor,
    }
    app.dependency_overrides.update(overrides)

    with TestClient(app, raise_server_exceptions=False) as c:
        # セッションにユーザーをセット
        c.cookies.set("session", "test-session")
        # 認証済みセッションを設定
        with c:
            # ログインしてセッションを確立
            login_page = c.get("/login")
            from bs4 import BeautifulSoup
            soup = BeautifulSoup(login_page.content, "html.parser")
            token_input = soup.find("input", {"name": "csrf_token"})
            csrf_token = token_input.get("value") if token_input else ""
            c.headers.update({"X-CSRFToken": csrf_token})
            c.post(
                "/login",
                data={"username": "admin", "password": "pass", "csrf_token": csrf_token},
                follow_redirects=False,
            )
            yield c

    for key in overrides:
        app.dependency_overrides.pop(key, None)


def _make_file(content: bytes, filename: str, content_type: str):
    """テスト用ファイルオブジェクトを作成するヘルパー"""
    return (filename, io.BytesIO(content), content_type)


class TestUploadFileValidation:
    """ファイルアップロードバリデーションテスト"""

    def test_valid_jpeg_upload_accepted(self, upload_client: TestClient):
        """有効なJPEGファイルのアップロードは受け付けられる"""
        # 最小限の有効なJPEGヘッダー
        jpeg_data = b"\xff\xd8\xff\xe0" + b"\x00" * 10
        response = upload_client.post(
            "/api/post",
            data={"text": "テスト投稿", "sns_targets": "x-main"},
            files={"media_files": _make_file(jpeg_data, "test.jpg", "image/jpeg")},
        )
        assert response.status_code in (200, 202), f"予期しないステータス: {response.status_code}"

    def test_valid_png_upload_accepted(self, upload_client: TestClient):
        """有効なPNGファイルのアップロードは受け付けられる"""
        png_data = b"\x89PNG\r\n\x1a\n" + b"\x00" * 10
        response = upload_client.post(
            "/api/post",
            data={"text": "テスト投稿", "sns_targets": "x-main"},
            files={"media_files": _make_file(png_data, "test.png", "image/png")},
        )
        assert response.status_code in (200, 202), f"予期しないステータス: {response.status_code}"

    def test_invalid_mime_type_rejected(self, upload_client: TestClient):
        """許可されていないMIME型（実行ファイル）は拒否される"""
        exe_data = b"MZ" + b"\x00" * 100  # PEヘッダー（Windows実行ファイル）
        response = upload_client.post(
            "/api/post",
            data={"text": "テスト投稿", "sns_targets": "x-main"},
            files={"media_files": _make_file(exe_data, "malware.exe", "application/octet-stream")},
        )
        assert response.status_code == 400, f"実行ファイルは拒否されるべき: {response.status_code}"

    def test_script_file_rejected(self, upload_client: TestClient):
        """スクリプトファイル（text/x-python）は拒否される"""
        script_data = b"import os; os.system('rm -rf /')"
        response = upload_client.post(
            "/api/post",
            data={"text": "テスト投稿", "sns_targets": "x-main"},
            files={"media_files": _make_file(script_data, "evil.py", "text/x-python")},
        )
        assert response.status_code == 400, f"スクリプトファイルは拒否されるべき: {response.status_code}"

    def test_file_too_large_rejected(self, upload_client: TestClient):
        """サイズ制限（10MB）を超えるファイルは拒否される"""
        large_data = b"\xff\xd8\xff\xe0" + b"\x00" * (11 * 1024 * 1024)  # 11MB
        response = upload_client.post(
            "/api/post",
            data={"text": "テスト投稿", "sns_targets": "x-main"},
            files={"media_files": _make_file(large_data, "huge.jpg", "image/jpeg")},
        )
        assert response.status_code == 400, f"大きすぎるファイルは拒否されるべき: {response.status_code}"

    def test_gif_upload_accepted(self, upload_client: TestClient):
        """GIFファイルのアップロードは受け付けられる"""
        gif_data = b"GIF89a" + b"\x00" * 10
        response = upload_client.post(
            "/api/post",
            data={"text": "テスト投稿", "sns_targets": "x-main"},
            files={"media_files": _make_file(gif_data, "anim.gif", "image/gif")},
        )
        assert response.status_code in (200, 202), f"予期しないステータス: {response.status_code}"

    def test_html_file_rejected(self, upload_client: TestClient):
        """HTMLファイル（XSS攻撃ベクタ）は拒否される"""
        html_data = b"<html><script>alert('xss')</script></html>"
        response = upload_client.post(
            "/api/post",
            data={"text": "テスト投稿", "sns_targets": "x-main"},
            files={"media_files": _make_file(html_data, "page.html", "text/html")},
        )
        assert response.status_code == 400, f"HTMLファイルは拒否されるべき: {response.status_code}"


class TestUploadValidationUnit:
    """アップロードバリデーションユニットテスト"""

    def test_validate_allowed_mime_types(self):
        """許可されたMIME型は検証を通過する"""
        from src.web.upload_validator import validate_upload_file

        allowed_types = ["image/jpeg", "image/png", "image/gif", "image/webp"]
        for mime_type in allowed_types:
            mock_file = MagicMock()
            mock_file.content_type = mime_type
            mock_file.size = 1024
            mock_file.filename = f"test.jpg"
            error = validate_upload_file(mock_file)
            assert error is None, f"{mime_type} は許可されるべき: {error}"

    def test_validate_disallowed_mime_types(self):
        """許可されていないMIME型はエラーを返す"""
        from src.web.upload_validator import validate_upload_file

        disallowed_types = [
            "application/octet-stream",
            "text/html",
            "text/x-python",
            "application/javascript",
            "application/x-sh",
        ]
        for mime_type in disallowed_types:
            mock_file = MagicMock()
            mock_file.content_type = mime_type
            mock_file.size = 1024
            mock_file.filename = "test.bin"
            error = validate_upload_file(mock_file)
            assert error is not None, f"{mime_type} は拒否されるべき"

    def test_validate_file_size_limit(self):
        """ファイルサイズ制限（10MB）の検証"""
        from src.web.upload_validator import validate_upload_file, MAX_UPLOAD_SIZE_BYTES

        mock_file = MagicMock()
        mock_file.content_type = "image/jpeg"
        mock_file.filename = "test.jpg"

        # ちょうど制限内
        mock_file.size = MAX_UPLOAD_SIZE_BYTES
        assert validate_upload_file(mock_file) is None

        # 制限超過
        mock_file.size = MAX_UPLOAD_SIZE_BYTES + 1
        error = validate_upload_file(mock_file)
        assert error is not None, "サイズ超過はエラーになるべき"

    def test_validate_no_filename(self):
        """ファイル名なしは許容する（スキップ対象）"""
        from src.web.upload_validator import validate_upload_file

        mock_file = MagicMock()
        mock_file.filename = ""
        mock_file.content_type = "image/jpeg"
        mock_file.size = 1024
        # ファイル名なしはスキップ（エラーなし）
        error = validate_upload_file(mock_file)
        assert error is None
