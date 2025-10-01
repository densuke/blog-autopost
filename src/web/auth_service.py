#!/usr/bin/env python
# -*- coding: utf-8 -*-

from ..config_manager import ConfigManager

class AuthService:
    def __init__(self, config_manager: ConfigManager):
        self.config_manager = config_manager

    def verify_credentials(self, username, password) -> bool:
        """提供された認証情報が設定ファイルのものと一致するか検証する"""
        credentials = self.config_manager.get_web_auth_credentials()
        if not credentials:
            return False
        
        correct_username = credentials.get('username')
        correct_password = credentials.get('password')

        return username == correct_username and password == correct_password
