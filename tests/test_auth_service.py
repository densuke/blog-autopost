#!/usr/bin/env python
# -*- coding: utf-8 -*-

import bcrypt
import pytest
from unittest.mock import MagicMock, call


def _make_hash(password: str) -> str:
    """テスト用にbcryptハッシュを生成するヘルパー"""
    return bcrypt.hashpw(password.encode(), bcrypt.gensalt()).decode()


@pytest.fixture
def mock_config_manager_plain():
    """平文パスワードを持つConfigManagerのモック"""
    mock = MagicMock()
    mock.get_web_auth_credentials.return_value = {
        'username': 'testuser',
        'password': 'testpassword',
    }
    return mock


@pytest.fixture
def mock_config_manager_hashed():
    """bcryptハッシュ済みパスワードを持つConfigManagerのモック"""
    mock = MagicMock()
    mock.get_web_auth_credentials.return_value = {
        'username': 'testuser',
        'password': _make_hash('testpassword'),
    }
    return mock


# --- 後方互換：既存テストの名前を残す ---

@pytest.fixture
def mock_config_manager(mock_config_manager_plain):
    """既存テストとの互換性を維持するフィクスチャ"""
    return mock_config_manager_plain


def test_verify_credentials_success(mock_config_manager):
    """正しい認証情報で認証が成功することをテストする（平文パスワード）"""
    from src.web.auth_service import AuthService
    auth_service = AuthService(mock_config_manager)
    assert auth_service.verify_credentials('testuser', 'testpassword') is True


def test_verify_credentials_failure(mock_config_manager):
    """間違った認証情報で認証が失敗することをテストする"""
    from src.web.auth_service import AuthService
    auth_service = AuthService(mock_config_manager)
    assert auth_service.verify_credentials('testuser', 'wrongpassword') is False
    assert auth_service.verify_credentials('wronguser', 'testpassword') is False


# --- 新規：bcrypt移行テスト ---

def test_verify_credentials_with_hashed_password(mock_config_manager_hashed):
    """ハッシュ済みパスワードで認証が成功することをテストする"""
    from src.web.auth_service import AuthService
    auth_service = AuthService(mock_config_manager_hashed)
    assert auth_service.verify_credentials('testuser', 'testpassword') is True


def test_verify_credentials_with_hashed_password_failure(mock_config_manager_hashed):
    """ハッシュ済みパスワードで誤ったパスワードは拒否されることをテストする"""
    from src.web.auth_service import AuthService
    auth_service = AuthService(mock_config_manager_hashed)
    assert auth_service.verify_credentials('testuser', 'wrongpassword') is False


def test_plain_password_triggers_migration(mock_config_manager_plain):
    """平文パスワードで認証成功時に自動でハッシュ化・保存が呼ばれることをテストする"""
    from src.web.auth_service import AuthService
    auth_service = AuthService(mock_config_manager_plain)

    result = auth_service.verify_credentials('testuser', 'testpassword')

    assert result is True
    # config_managerのupdate_web_auth_passwordが呼ばれることを確認
    mock_config_manager_plain.update_web_auth_password.assert_called_once()
    # 引数がbcryptハッシュ形式であることを確認
    saved_hash = mock_config_manager_plain.update_web_auth_password.call_args[0][0]
    assert saved_hash.startswith('$2b$'), "保存されたパスワードはbcryptハッシュであるべき"
    assert bcrypt.checkpw(b'testpassword', saved_hash.encode()), "保存されたハッシュで元のパスワードを検証できるべき"


def test_plain_password_wrong_does_not_trigger_migration(mock_config_manager_plain):
    """間違った平文パスワードでは移行処理が呼ばれないことをテストする"""
    from src.web.auth_service import AuthService
    auth_service = AuthService(mock_config_manager_plain)

    result = auth_service.verify_credentials('testuser', 'wrongpassword')

    assert result is False
    mock_config_manager_plain.update_web_auth_password.assert_not_called()


def test_hashed_password_does_not_trigger_migration(mock_config_manager_hashed):
    """ハッシュ済みパスワードでは移行処理が呼ばれないことをテストする"""
    from src.web.auth_service import AuthService
    auth_service = AuthService(mock_config_manager_hashed)

    result = auth_service.verify_credentials('testuser', 'testpassword')

    assert result is True
    mock_config_manager_hashed.update_web_auth_password.assert_not_called()


def test_no_credentials_returns_false():
    """認証情報が存在しない場合はFalseを返すことをテストする"""
    from src.web.auth_service import AuthService
    mock = MagicMock()
    mock.get_web_auth_credentials.return_value = None
    auth_service = AuthService(mock)
    assert auth_service.verify_credentials('testuser', 'testpassword') is False
