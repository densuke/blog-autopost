#!/usr/bin/env python
# -*- coding: utf-8 -*-

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

# appをインポートする前に、テスト用の設定をロードする必要があるかもしれない
# 現時点では直接インポートする
from src.web.main_web import app

client = TestClient(app)

def test_fastapi_instance():
    """src.web.main_webにFastAPIインスタンスが存在することを確認する"""
    assert isinstance(app, FastAPI)

def test_get_login_page():
    """/loginエンドポイントがログインフォームを返すことをテストする"""
    response = client.get("/login")
    assert response.status_code == 200
    assert b'<form' in response.content
    assert b'name="username"' in response.content
    assert b'name="password"' in response.content
