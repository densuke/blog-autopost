#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""config_managerのセキュリティ機能（パーミッション・パスワード更新）テスト"""

import os
import stat
import tempfile
from pathlib import Path

import pytest
import yaml


@pytest.fixture
def config_file_factory(tmp_path):
    """テスト用config.ymlを生成するファクトリ"""
    def _factory(mode: int = 0o600, content: dict | None = None) -> Path:
        cfg = content or {
            'web_auth': {
                'username': 'admin',
                'password': 'testpass',
                'secret_key': 'dummy-secret',
            }
        }
        path = tmp_path / "config.yml"
        path.write_text(yaml.dump(cfg), encoding='utf-8')
        path.chmod(mode)
        return path
    return _factory


class TestLoadConfigPermission:
    """load_config()のパーミッションチェック動作テスト"""

    def test_secure_permission_loads_normally(self, config_file_factory):
        """600パーミッションのファイルは警告なしで読み込めることをテストする"""
        from src.config_manager import load_config
        path = config_file_factory(mode=0o600)
        config = load_config(str(path))
        assert config['web_auth']['username'] == 'admin'
        # パーミッションが変わっていないことを確認
        assert stat.S_IMODE(path.stat().st_mode) == 0o600

    def test_too_open_permission_is_fixed(self, config_file_factory, caplog):
        """644など緩いパーミッションのファイルは600に修正されることをテストする"""
        import logging
        from src.config_manager import load_config
        path = config_file_factory(mode=0o644)

        with caplog.at_level(logging.WARNING):
            config = load_config(str(path))

        # 読み込み自体は成功
        assert config['web_auth']['username'] == 'admin'
        # パーミッションが600に修正されていること
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        # 警告ログが出ていること
        assert any('パーミッション' in r.message or 'permission' in r.message.lower()
                   for r in caplog.records)

    def test_world_readable_permission_is_fixed(self, config_file_factory):
        """777など全開放パーミッションも600に修正されることをテストする"""
        from src.config_manager import load_config
        path = config_file_factory(mode=0o777)
        load_config(str(path))
        assert stat.S_IMODE(path.stat().st_mode) == 0o600

    def test_read_only_permission_is_not_changed(self, config_file_factory):
        """400（読み取り専用）は緩くないので変更されないことをテストする"""
        from src.config_manager import load_config
        path = config_file_factory(mode=0o400)
        load_config(str(path))
        assert stat.S_IMODE(path.stat().st_mode) == 0o400


class TestUpdateWebAuthPassword:
    """ConfigManager.update_web_auth_password()のテスト"""

    def test_update_password_writes_to_file(self, config_file_factory):
        """ハッシュ済みパスワードをconfig.ymlに書き込めることをテストする"""
        from src.config_manager import ConfigManager
        path = config_file_factory(mode=0o600)
        manager = ConfigManager(str(path))

        manager.update_web_auth_password('$2b$12$hashedvalue')

        # ファイルに書き込まれたことを確認
        saved = yaml.safe_load(path.read_text(encoding='utf-8'))
        assert saved['web_auth']['password'] == '$2b$12$hashedvalue'

    def test_update_password_keeps_other_fields(self, config_file_factory):
        """パスワード更新時に他のフィールドが保持されることをテストする"""
        from src.config_manager import ConfigManager
        path = config_file_factory(mode=0o600, content={
            'web_auth': {
                'username': 'myuser',
                'password': 'oldpass',
                'secret_key': 'mysecret',
            },
            'other_section': {'key': 'value'},
        })
        manager = ConfigManager(str(path))

        manager.update_web_auth_password('$2b$12$newhash')

        saved = yaml.safe_load(path.read_text(encoding='utf-8'))
        assert saved['web_auth']['username'] == 'myuser'
        assert saved['web_auth']['secret_key'] == 'mysecret'
        assert saved['other_section']['key'] == 'value'

    def test_update_password_preserves_file_permission(self, config_file_factory):
        """パスワード更新後もファイルのパーミッションが600を維持することをテストする"""
        from src.config_manager import ConfigManager
        path = config_file_factory(mode=0o600)
        manager = ConfigManager(str(path))

        manager.update_web_auth_password('$2b$12$hashedvalue')

        assert stat.S_IMODE(path.stat().st_mode) == 0o600

    def test_update_password_also_updates_in_memory(self, config_file_factory):
        """パスワード更新後にget_web_auth_credentialsが新しいハッシュを返すことをテストする"""
        from src.config_manager import ConfigManager
        path = config_file_factory(mode=0o600)
        manager = ConfigManager(str(path))

        manager.update_web_auth_password('$2b$12$hashedvalue')

        creds = manager.get_web_auth_credentials()
        assert creds['password'] == '$2b$12$hashedvalue'
