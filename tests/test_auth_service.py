#!/usr/bin/env python
# -*- coding: utf-8 -*-

import pytest
from unittest.mock import MagicMock


@pytest.fixture
def mock_config_manager():
    """ConfigManagerのモックを作成するフィクスチャ"""
    mock = MagicMock()
    mock.get_web_auth_credentials.return_value = {
        'username': 'testuser',
        'password': 'testpassword'
    }
    return mock

# AuthServiceが存在しないため、このテストは最初は実行できない
# が、先にテストコードを記述する
def test_verify_credentials_success(mock_config_manager):
    """正しい認証情報で認証が成功することをテストする"""
    from src.web.auth_service import AuthService
    auth_service = AuthService(mock_config_manager)
    assert auth_service.verify_credentials('testuser', 'testpassword') is True

def test_verify_credentials_failure(mock_config_manager):
    """間違った認証情報で認証が失敗することをテストする"""
    from src.web.auth_service import AuthService
    auth_service = AuthService(mock_config_manager)
    assert auth_service.verify_credentials('testuser', 'wrongpassword') is False
    assert auth_service.verify_credentials('wronguser', 'testpassword') is False
