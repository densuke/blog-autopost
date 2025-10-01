#!/usr/bin/env python
# -*- coding: utf-8 -*-

from fastapi import FastAPI, Request, Depends, Form, HTTPException, status
from fastapi.responses import RedirectResponse
from fastapi.templating import Jinja2Templates
from starlette.middleware.sessions import SessionMiddleware

from ..config_manager import ConfigManager
from .auth_service import AuthService

app = FastAPI()

# 設定と認証サービスのインスタンス化
config_manager = ConfigManager("config.yml")
auth_service = AuthService(config_manager)

# セッション管理ミドルウェアの追加
# 秘密鍵は実際のアプリケーションでは環境変数などから読み込むべき
app.add_middleware(SessionMiddleware, secret_key=config_manager.get_secret_key())

templates = Jinja2Templates(directory="src/web/templates")

@app.get("/")
def read_root(request: Request):
    user = request.session.get('user')
    if not user:
        return RedirectResponse(url="/login", status_code=status.HTTP_303_SEE_OTHER)
    
    sns_configs = config_manager.get_all_sns_configs()
    # 設定から名前とタイプを抽出する
    sns_accounts = []
    if isinstance(sns_configs, list):
        for config in sns_configs:
            sns_accounts.append({'name': config.get('name'), 'type': config.get('type')})
    elif isinstance(sns_configs, dict):
        for name, config in sns_configs.items():
            sns_accounts.append({'name': name, 'type': name})

    return templates.TemplateResponse("index.html", {"request": request, "user": user, "sns_accounts": sns_accounts})

@app.get("/login")
def login_form(request: Request):
    return templates.TemplateResponse("login.html", {"request": request})

@app.post("/login")
def login_submit(request: Request, username: str = Form(...), password: str = Form(...)):
    if auth_service.verify_credentials(username, password):
        request.session['user'] = username
        return RedirectResponse(url="/", status_code=status.HTTP_303_SEE_OTHER)
    else:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Incorrect username or password",
        )

@app.get("/logout")
def logout(request: Request):
    request.session.clear()
    return RedirectResponse(url="/login", status_code=status.HTTP_303_SEE_OTHER)
