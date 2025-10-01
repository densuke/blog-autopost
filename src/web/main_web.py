from fastapi import FastAPI, Request, Depends, Form, HTTPException, status, File, UploadFile
from fastapi.responses import RedirectResponse, JSONResponse
from fastapi.templating import Jinja2Templates
from starlette.middleware.sessions import SessionMiddleware
from typing import List
import shutil
import tempfile
import os

from ..config_manager import ConfigManager
from .auth_service import AuthService
from ..media_validator import MediaValidator
from ..image_resizer import ImageResizer
from ..text_optimizer import TextOptimizer
from .posting_service import PostingService
from apscheduler.schedulers.background import BackgroundScheduler
from apscheduler.jobstores.sqlalchemy import SQLAlchemyJobStore

app = FastAPI()

# データディレクトリの確認と作成
DATA_DIR = "data"
if not os.path.exists(DATA_DIR):
    os.makedirs(DATA_DIR)

# スケジューラの設定
jobstores = {
    'default': SQLAlchemyJobStore(url=f'sqlite:///{DATA_DIR}/jobs.sqlite')
}
scheduler = BackgroundScheduler(jobstores=jobstores)

@app.on_event("startup")
def startup_event():
    scheduler.start()

@app.on_event("shutdown")
def shutdown_event():
    scheduler.shutdown()


# 設定と認証サービスのインスタンス化
config_manager = ConfigManager("config.yml")
auth_service = AuthService(config_manager)

# 投稿関連サービスのインスタンス化
# media_validator = MediaValidator() # PostingService内で直接呼び出すため不要
image_resizer = ImageResizer()
text_optimizer = TextOptimizer(config_manager.config)
posting_service = PostingService(
    config_manager=config_manager, 
    image_resizer=image_resizer, 
    text_optimizer=text_optimizer
)

# セッション管理ミドルウェアの追加
app.add_middleware(SessionMiddleware, secret_key=config_manager.get_secret_key())

templates = Jinja2Templates(directory="src/web/templates")

# 認証チェック用のDependency
def get_current_user(request: Request):
    user = request.session.get("user")
    if not user:
        raise HTTPException(
            status_code=status.HTTP_303_SEE_OTHER,
            detail="Not authenticated",
            headers={"Location": "/login"},
        )
    return user

@app.get("/")
def read_root(request: Request, user: str = Depends(get_current_user)):
    sns_configs = config_manager.get_all_sns_configs()
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

@app.post("/api/post")
def api_post(
    request: Request,
    text: str = Form(...),
    url: str = Form(None),
    sns_targets: List[str] = Form(...),
    media_files: List[UploadFile] = File([]),
    user: str = Depends(get_current_user)
):
    temp_dir = tempfile.mkdtemp()
    media_paths = []
    try:
        for file in media_files:
            path = os.path.join(temp_dir, file.filename)
            with open(path, "wb") as buffer:
                shutil.copyfileobj(file.file, buffer)
            media_paths.append(path)

        post_data = {
            'text': text,
            'url': url,
            'sns_targets': sns_targets,
            'media_files': media_paths
        }

        result = posting_service.post_now(post_data)
        return JSONResponse(content=result)

    finally:
        shutil.rmtree(temp_dir)

@app.post("/api/schedule")
def api_schedule(
    request: Request,
    text: str = Form(...),
    url: str = Form(None),
    sns_targets: List[str] = Form(...),
    media_files: List[UploadFile] = File([]),
    schedule_time: str = Form(...),
    user: str = Depends(get_current_user)
):
    from datetime import datetime

    temp_dir = tempfile.mkdtemp()
    media_paths = []
    try:
        for file in media_files:
            path = os.path.join(temp_dir, file.filename)
            with open(path, "wb") as buffer:
                shutil.copyfileobj(file.file, buffer)
            media_paths.append(path)

        post_data = {
            'text': text,
            'url': url,
            'sns_targets': sns_targets,
            'media_files': media_paths
        }

        run_date = datetime.fromisoformat(schedule_time)

        scheduler.add_job(posting_service.post_now, 'date', run_date=run_date, args=[post_data])
        
        return JSONResponse(content={"message": "Post scheduled successfully"})

    except Exception as e:
        # In a real app, you'd want more specific error handling
        raise HTTPException(status_code=400, detail=str(e))
    # Note: The temporary directory with files for scheduled posts is not cleaned up here.
    # This is a simplification for this implementation step.
    # A more robust solution would involve a persistent storage for uploaded files.
