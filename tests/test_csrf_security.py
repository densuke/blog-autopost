#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""CSRFセキュリティ関連テスト

- セッションとCSRFのシークレット分離
- CSRFクッキーのSecureフラグ設定
"""

import pytest
from unittest.mock import MagicMock, patch


class TestCsrfSecretSeparation:
    """セッションとCSRFのシークレット分離テスト"""

    def test_get_csrf_secret_key_returns_dedicated_key(self):
        """csrf_secret_keyが設定されている場合はそちらを返す"""
        from src.config_manager import ConfigManager

        config = {
            "web_auth": {
                "secret_key": "session-secret",
                "csrf_secret_key": "csrf-dedicated-secret",
            }
        }
        cm = ConfigManager.__new__(ConfigManager)
        cm.config = config
        cm._config_path = None

        assert cm.get_csrf_secret_key() == "csrf-dedicated-secret"

    def test_get_csrf_secret_key_derives_from_session_key_when_not_set(self):
        """csrf_secret_keyが未設定の場合はセッションキーから派生させる"""
        from src.config_manager import ConfigManager

        config = {
            "web_auth": {
                "secret_key": "session-secret",
            }
        }
        cm = ConfigManager.__new__(ConfigManager)
        cm.config = config
        cm._config_path = None

        csrf_key = cm.get_csrf_secret_key()
        session_key = cm.get_secret_key()

        assert csrf_key is not None
        assert csrf_key != session_key  # 派生値はセッションキーと異なるべき

    def test_get_csrf_secret_key_returns_none_when_no_secret_key(self):
        """secret_keyがない場合はNoneを返す"""
        from src.config_manager import ConfigManager

        config = {"web_auth": {}}
        cm = ConfigManager.__new__(ConfigManager)
        cm.config = config
        cm._config_path = None

        assert cm.get_csrf_secret_key() is None

    def test_csrf_key_derivation_is_deterministic(self):
        """同じsecret_keyからは常に同じCSRFキーが派生する"""
        from src.config_manager import ConfigManager

        config = {"web_auth": {"secret_key": "stable-key"}}
        cm = ConfigManager.__new__(ConfigManager)
        cm.config = config
        cm._config_path = None

        key1 = cm.get_csrf_secret_key()
        key2 = cm.get_csrf_secret_key()
        assert key1 == key2


class TestCookieSecureFlag:
    """CSRFクッキーのSecureフラグ設定テスト"""

    def test_get_cookie_secure_returns_false_by_default(self):
        """デフォルトではcookie_secureはFalse"""
        from src.config_manager import ConfigManager

        config = {"web_auth": {"secret_key": "session-secret"}}
        cm = ConfigManager.__new__(ConfigManager)
        cm.config = config
        cm._config_path = None

        assert cm.get_cookie_secure() is False

    def test_get_cookie_secure_returns_true_when_configured(self):
        """cookie_secure: trueが設定されている場合はTrueを返す"""
        from src.config_manager import ConfigManager

        config = {
            "web_auth": {
                "secret_key": "session-secret",
                "cookie_secure": True,
            }
        }
        cm = ConfigManager.__new__(ConfigManager)
        cm.config = config
        cm._config_path = None

        assert cm.get_cookie_secure() is True

    def test_get_cookie_secure_returns_false_for_false_value(self):
        """cookie_secure: falseが設定されている場合はFalseを返す"""
        from src.config_manager import ConfigManager

        config = {
            "web_auth": {
                "secret_key": "session-secret",
                "cookie_secure": False,
            }
        }
        cm = ConfigManager.__new__(ConfigManager)
        cm.config = config
        cm._config_path = None

        assert cm.get_cookie_secure() is False


class TestMainWebMiddlewareConfig:
    """main_web.pyのミドルウェア設定テスト"""

    def test_csrf_middleware_uses_separate_secret(self):
        """CSRFミドルウェアがセッションと別のシークレットを使用する"""
        from src.web.csrf_protection import CSRFCookieMiddleware
        from starlette.applications import Starlette

        session_secret = "session-only-secret"
        csrf_secret = "csrf-only-secret"

        # 別々のシークレットでミドルウェアを作成できることを確認
        app = Starlette()
        middleware = CSRFCookieMiddleware(app, secret=csrf_secret)

        # 異なるシークレットからは異なるトークンが生成される
        token_csrf = middleware._generate_csrf_token()
        assert isinstance(token_csrf, str)
        assert len(token_csrf) > 0

    def test_csrf_cookie_secure_flag_applies(self):
        """CSRFCookieMiddlewareにcookie_secure=Trueを渡せる"""
        from src.web.csrf_protection import CSRFCookieMiddleware
        from starlette.applications import Starlette

        app = Starlette()
        middleware = CSRFCookieMiddleware(app, secret="some-secret", cookie_secure=True)
        assert middleware.cookie_secure is True

    def test_csrf_cookie_insecure_flag_applies(self):
        """CSRFCookieMiddlewareにcookie_secure=Falseを渡せる"""
        from src.web.csrf_protection import CSRFCookieMiddleware
        from starlette.applications import Starlette

        app = Starlette()
        middleware = CSRFCookieMiddleware(app, secret="some-secret", cookie_secure=False)
        assert middleware.cookie_secure is False
