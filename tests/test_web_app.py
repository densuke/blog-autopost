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

def test_get_main_page_authenticated():
    """認証済みユーザーがメインページにアクセスでき、投稿フォームが表示されることをテストする"""
    client = TestClient(app)
    # ログイン
    client.post("/login", data={"username": "admin", "password": "your_strong_password_here"})

    response = client.get("/")
    assert response.status_code == 200
    # <textarea>タグとname="text"属性の存在を個別にチェック
    assert b'<textarea' in response.content
    assert b'name="text"' in response.content
    # config.ymlに設定されているSNSアカウントのチェックボックスが表示されることを確認
    assert b'<input type="checkbox"' in response.content
    assert b'name="sns_targets"' in response.content
    assert b'value="x-main"' in response.content

from unittest.mock import patch

def test_post_api_endpoint():
    """/api/postエンドポイントがPostingServiceを正しく呼び出すことをテストする"""
    client = TestClient(app)
    # ログイン
    client.post("/login", data={"username": "admin", "password": "your_strong_password_here"})

    with patch('src.web.main_web.posting_service') as mock_posting_service:
        mock_posting_service.post_now.return_value = {'x-main': {'success': True}}
        
        # ダミーのアップロードファイルを作成
        dummy_file_content = b"dummy image content"
        files = {'media_files': ('test.jpg', dummy_file_content, 'image/jpeg')}
        data = {
            'text': 'Test post',
            'url': 'http://example.com',
            'sns_targets': 'x-main'
        }

        response = client.post("/api/post", data=data, files=files)

        assert response.status_code == 200
        assert response.json() == {'x-main': {'success': True}}

        # PostingServiceが正しい引数で呼び出されたか検証
        # ファイルパスは一時的なものなので、ここでは呼び出されたこと自体を主眼に置く
        mock_posting_service.post_now.assert_called_once()
        called_args, _ = mock_posting_service.post_now.call_args
        assert called_args[0]['text'] == 'Test post'
        assert called_args[0]['url'] == 'http://example.com'
        assert called_args[0]['sns_targets'] == ['x-main']
        assert len(called_args[0]['media_files']) == 1

def test_scheduler_lifecycle():
    """アプリケーションのライフサイクルでスケジューラが開始・停止されることをテストする"""
    with patch('src.web.main_web.scheduler') as mock_scheduler:
        with TestClient(app) as client:
            # アプリケーションの起動時にスケジューラが開始されることを確認
            mock_scheduler.start.assert_called_once()
        # アプリケーションの終了時にスケジューラがシャットダウンされることを確認
        mock_scheduler.shutdown.assert_called_once()
