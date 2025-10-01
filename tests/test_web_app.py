#!/usr/bin/env python
# -*- coding: utf-8 -*-

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from src.web.main_web import app

def test_fastapi_instance():
    """src.web.main_webにFastAPIインスタンスが存在することを確認する"""
    assert isinstance(app, FastAPI)

def test_get_login_page():
    """/loginエンドポイントがログインフォームを返すことをテストする"""
    client = TestClient(app)
    response = client.get("/login")
    assert response.status_code == 200
    assert b'<form' in response.content
    assert b'name="username"' in response.content
    assert b'name="password"' in response.content

def test_login_success():
    """正しい認証情報でログインが成功し、リダイレクトとセッション設定が行われることをテストする"""
    client = TestClient(app)
    response = client.post(
        "/login", 
        data={"username": "admin", "password": "your_strong_password_here"},
        follow_redirects=False
    )
    assert response.status_code == 303  # See Other, for redirect after POST
    assert response.headers["location"] == "/"
    assert "session" in response.cookies

def test_login_failure():
    """間違った認証情報でログインが失敗することをテストする"""
    client = TestClient(app)
    response = client.post(
        "/login", 
        data={"username": "admin", "password": "wrongpassword"},
        follow_redirects=False
    )
    assert response.status_code == 401
    assert "session" not in response.cookies

def test_logout():
    """ログアウト後、保護されたルートにアクセスするとリダイレクトされることをテストする"""
    client = TestClient(app)
    # まずログインする
    login_response = client.post("/login", data={"username": "admin", "password": "your_strong_password_here"}, follow_redirects=True)
    assert login_response.status_code == 200 # ルートへのリダイレクト成功を確認

    # ログアウト
    logout_response = client.get("/logout", follow_redirects=False)
    assert logout_response.status_code == 303
    assert logout_response.headers["location"] == "/login"

    # 保護されたルートにアクセスを試みる
    response = client.get("/", follow_redirects=False)
    assert response.status_code == 303
    assert response.headers["location"] == "/login"

def test_access_root_unauthenticated():
    """未認証でルートにアクセスするとログインページにリダイレクトされることをテストする"""
    client = TestClient(app)
    response = client.get("/", follow_redirects=False)
    assert response.status_code == 303
    assert response.headers["location"] == "/login"
