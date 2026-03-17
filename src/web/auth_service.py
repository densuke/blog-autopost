#!/usr/bin/env python
# -*- coding: utf-8 -*-

import logging

import bcrypt

from ..config_manager import ConfigManager

logger = logging.getLogger(__name__)


def _is_bcrypt_hash(value: str) -> bool:
    """bcryptハッシュ形式（$2b$, $2a$, $2y$ で始まる）かどうかを判定する。"""
    return value.startswith(('$2b$', '$2a$', '$2y$'))


class AuthService:
    def __init__(self, config_manager: ConfigManager):
        self.config_manager = config_manager

    def verify_credentials(self, username: str, password: str) -> bool:
        """
        提供された認証情報が設定ファイルのものと一致するか検証する。

        パスワードが平文の場合、認証成功時に自動でbcryptハッシュへ移行し
        config.yml を書き換える。次回以降はハッシュで照合される。

        Args:
            username (str): 入力されたユーザー名
            password (str): 入力されたパスワード（平文）

        Returns:
            bool: 認証に成功した場合 True
        """
        credentials = self.config_manager.get_web_auth_credentials()
        if not credentials:
            return False

        correct_username = credentials.get('username')
        stored_password = credentials.get('password', '')

        if username != correct_username:
            return False

        if _is_bcrypt_hash(stored_password):
            return bcrypt.checkpw(password.encode(), stored_password.encode())

        # 平文パスワード：照合して成功したら自動でハッシュ化移行
        if password == stored_password:
            logger.warning(
                "平文パスワードを検出しました。bcryptハッシュに自動移行します。"
            )
            hashed = bcrypt.hashpw(password.encode(), bcrypt.gensalt()).decode()
            self.config_manager.update_web_auth_password(hashed)
            return True

        return False
