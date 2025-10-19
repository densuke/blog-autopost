"""認証関連ルート"""
from fastapi import APIRouter, Depends, Form, HTTPException, Request, status
from fastapi.responses import RedirectResponse
from fastapi.templating import Jinja2Templates

from ..auth_service import AuthService
from ..dependencies import get_auth_service, get_csrf_token, get_templates

router = APIRouter()


@router.get("/login")
def login_form(
    request: Request,
    templates: Jinja2Templates = Depends(get_templates)
):
    """ログインフォーム表示"""
    return templates.TemplateResponse(
        "login.html",
        {
            "request": request,
            "csrf_token": get_csrf_token(request),
        },
    )


@router.post("/login")
def login_submit(
    request: Request,
    username: str = Form(...),
    password: str = Form(...),
    auth_service: AuthService = Depends(get_auth_service)
):
    """ログイン処理"""
    if auth_service.verify_credentials(username, password):
        request.session['user'] = username
        return RedirectResponse(url="/", status_code=status.HTTP_303_SEE_OTHER)
    else:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Incorrect username or password",
        )


@router.get("/logout")
def logout(request: Request):
    """ログアウト処理"""
    request.session.clear()
    return RedirectResponse(url="/login", status_code=status.HTTP_303_SEE_OTHER)
