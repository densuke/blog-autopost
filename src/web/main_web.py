from fastapi import FastAPI, Request, Depends, Form, HTTPException, status, File, UploadFile
from fastapi.responses import RedirectResponse, JSONResponse
from fastapi.templating import Jinja2Templates
from starlette.middleware.sessions import SessionMiddleware
from typing import List, Optional
import shutil
import tempfile
import os
from pathlib import Path
from datetime import datetime
import logging

from ..config_manager import ConfigManager
from .auth_service import AuthService
from ..media_validator import MediaValidator
from ..image_resizer import ImageResizer
from ..text_optimizer import TextOptimizer
from .posting_service import PostingService
from apscheduler.schedulers.background import BackgroundScheduler
from apscheduler.jobstores.sqlalchemy import SQLAlchemyJobStore

from .scheduled_post_store import ScheduledPostStore
from .scheduled_post_model import ScheduledPost
from .post_executor import PostExecutor
from .scheduler_service import SchedulerService

# ログの設定
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.FileHandler('data/blog_autopost.log'),
        logging.StreamHandler()
    ]
)
logger = logging.getLogger(__name__)

app = FastAPI()

# データディレクトリの確認と作成
DATA_DIR = os.environ.get("DATA_DIR", "data")
if not os.path.exists(DATA_DIR):
    os.makedirs(DATA_DIR)

# 予約投稿データストアのパス
SCHEDULED_POSTS_FILE = Path(DATA_DIR) / "scheduled_posts.json"
scheduled_post_store = ScheduledPostStore(SCHEDULED_POSTS_FILE)

# 設定と認証サービスのインスタンス化
config_manager = ConfigManager("config.yml")
auth_service = AuthService(config_manager)

# 投稿実行サービスとスケジューラーサービスのインスタンス化
post_executor = PostExecutor(scheduled_post_store, config_manager)
scheduler_service = SchedulerService(scheduled_post_store, post_executor, DATA_DIR)

@app.on_event("startup")
def startup_event():
    scheduler_service.start()

@app.on_event("shutdown")
def shutdown_event():
    scheduler_service.shutdown()


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
secret_key = config_manager.get_secret_key()
if not secret_key:
    raise RuntimeError("セッション管理用のsecret_keyが設定されていません。config.ymlを確認してください。")
app.add_middleware(SessionMiddleware, secret_key=secret_key)

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
            sns_type = config.get('type', name) # configにtypeがあればそれを使用、なければnameをtypeとする
            sns_accounts.append({'name': name, 'type': sns_type})

    scheduled_posts = scheduled_post_store.get_all_posts()
    # UTCで保存されている日時をローカルタイムゾーンに変換して表示
    for post in scheduled_posts:
        if post.scheduled_at.tzinfo:
            post.scheduled_at = post.scheduled_at.astimezone()

    return templates.TemplateResponse("index.html", {"request": request, "user": user, "sns_accounts": sns_accounts, "scheduled_posts": scheduled_posts, "now": datetime.now()})

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

@app.get("/api/posts", response_model=List[ScheduledPost])
def get_api_posts(sort_by: Optional[str] = 'date_asc', user: str = Depends(get_current_user)):
    """
    ソート順を指定して、すべての予約投稿をJSON形式で取得します。
    """
    return scheduled_post_store.get_all_posts(sort_by=sort_by)

@app.get("/api/scheduled-posts", response_model=List[ScheduledPost])
def get_all_scheduled_posts(user: str = Depends(get_current_user)):
    return scheduled_post_store.get_all_posts()

@app.get("/api/scheduled-posts/{post_id}", response_model=ScheduledPost)
def get_scheduled_post(post_id: str, user: str = Depends(get_current_user)):
    post = scheduled_post_store.get_post_by_id(post_id)
    if not post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")
    return post

@app.post("/api/scheduled-posts", response_model=ScheduledPost, status_code=status.HTTP_201_CREATED)
def create_scheduled_post(
    scheduled_at: datetime = Form(...),
    content: str = Form(...),
    media_files: List[UploadFile] = File([]),
    target_sns: List[str] = Form(...),
    user: str = Depends(get_current_user)
):
    # メディアファイルの安全な保存
    media_paths = []
    if media_files:
        # 予約投稿用のメディア保存ディレクトリを作成
        scheduled_media_dir = os.path.join(DATA_DIR, "scheduled_media")
        os.makedirs(scheduled_media_dir, exist_ok=True)
        
        # 今回の予約投稿に紐づくユニークなサブディレクトリを作成
        import uuid
        post_media_dir = os.path.join(scheduled_media_dir, str(uuid.uuid4()))
        os.makedirs(post_media_dir, exist_ok=True)
        
        # セキュリティ: ディレクトリパーミッションを設定（所有者のみアクセス可能）
        os.chmod(post_media_dir, 0o700)
        
        for file in media_files:
            # ファイル名のサニタイズ（パストラバーサル攻撃対策）
            safe_filename = os.path.basename(file.filename)
            path = os.path.join(post_media_dir, safe_filename)
            
            with open(path, "wb") as buffer:
                shutil.copyfileobj(file.file, buffer)
            
            # セキュリティ: ファイルパーミッションを設定（所有者のみ読み書き可能）
            os.chmod(path, 0o600)
            
            media_paths.append(path)

    # SNS制限違反チェック
    supported_sns = config_manager.get_all_sns_names()
    if not all(sns in supported_sns for sns in target_sns):
        raise HTTPException(status_code=400, detail="Unsupported SNS target specified")

    logger.info(f"User {user} creating scheduled post for {scheduled_at}")
    new_post = ScheduledPost(
        scheduled_at=scheduled_at,
        content=content,
        media_files=media_paths,
        target_sns=target_sns,
        created_at=datetime.now(),
        updated_at=datetime.now()
    )
    scheduled_post_store.create_post(new_post)
    return new_post

@app.put("/api/scheduled-posts/{post_id}", response_model=ScheduledPost)
def update_scheduled_post(
    post_id: str,
    scheduled_at: Optional[datetime] = Form(None),
    content: Optional[str] = Form(None),
    media_files: List[UploadFile] = File([]), # 更新時は既存のものをどう扱うか検討
    target_sns: Optional[List[str]] = Form(None), # Optionalに変更
    user: str = Depends(get_current_user)
):
    existing_post = scheduled_post_store.get_post_by_id(post_id)
    if not existing_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")
    
    # 実行済みまたは失敗した投稿は更新不可
    if existing_post.status in ["実行済み", "失敗"]:
        raise HTTPException(status_code=409, detail="Cannot update an already executed or failed post")

    updates = {}
    if scheduled_at:
        updates["scheduled_at"] = scheduled_at
    if content:
        updates["content"] = content
    if target_sns is not None: # target_snsがNoneでない場合のみ更新
        # SNS制限違反チェック
        supported_sns = config_manager.get_all_sns_names()
        if not all(sns in supported_sns for sns in target_sns):
            raise HTTPException(status_code=400, detail="Unsupported SNS target specified")
        updates["target_sns"] = target_sns
    # media_filesの更新ロジックは複雑になるため後で検討

    updated_post = scheduled_post_store.update_post(post_id, updates)
    if not updated_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found after update attempt")
    return updated_post

@app.delete("/api/scheduled-posts/{post_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_scheduled_post(post_id: str, user: str = Depends(get_current_user)):
    existing_post = scheduled_post_store.get_post_by_id(post_id)
    if not existing_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")
    
    # 実行済みまたは失敗した投稿は削除不可
    if existing_post.status in ["実行済み", "失敗"]:
        raise HTTPException(status_code=409, detail="Cannot delete an already executed or failed post")

    if not scheduled_post_store.delete_post(post_id):
        raise HTTPException(status_code=404, detail="Scheduled post not found for deletion")
    return

@app.post("/api/scheduled-posts/{post_id}/re-execute", response_model=ScheduledPost)
def re_execute_scheduled_post(post_id: str, user: str = Depends(get_current_user)):
    existing_post = scheduled_post_store.get_post_by_id(post_id)
    if not existing_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")
    
    # 成功済みの投稿は再実行不可
    if existing_post.status == "実行済み":
        raise HTTPException(status_code=409, detail="Cannot re-execute an already successful post")

    # 投稿日時を現在時刻+1分に更新し、ステータスを「予約済み」に戻す
    from datetime import timedelta
    updates = {
        "scheduled_at": datetime.now() + timedelta(minutes=1),
        "status": "予約済み",
        "error_message": None
    }
    updated_post = scheduled_post_store.update_post(post_id, updates)
    if not updated_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found for re-execution")
    
    # TODO: APSchedulerにジョブを再登録するロジックが必要
    return updated_post

@app.post("/api/scheduled-posts/{post_id}/send-now", response_model=ScheduledPost)
def send_scheduled_post_now(post_id: str, user: str = Depends(get_current_user)):
    existing_post = scheduled_post_store.get_post_by_id(post_id)
    if not existing_post:
        logger.warning(f"User {user} attempted to send non-existent post {post_id}")
        raise HTTPException(status_code=404, detail="Scheduled post not found")
    
    # 実行済みの投稿は即時送信不可
    if existing_post.status == "実行済み":
        logger.warning(f"User {user} attempted to send already executed post {post_id}")
        raise HTTPException(status_code=409, detail="Cannot send an already executed post immediately")

    logger.info(f"User {user} requested immediate sending of post {post_id}")
    # PostExecutorを使って投稿を実行
    success = post_executor.execute_post(post_id, debug=True)
    
    # 更新された投稿を取得して返す
    updated_post = scheduled_post_store.get_post_by_id(post_id)
    if not updated_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found after immediate sending")
    
    if success:
        logger.info(f"Post {post_id} sent immediately successfully by user {user}")
    else:
        logger.error(f"Post {post_id} failed to send immediately for user {user}")
    
    return updated_post

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
    import uuid # ユニークなディレクトリ名生成用

    # 予約投稿用のメディア保存ディレクトリ
    SCHEDULED_MEDIA_DIR = os.path.join(DATA_DIR, "scheduled_media")
    os.makedirs(SCHEDULED_MEDIA_DIR, exist_ok=True)

    # 今回の予約投稿に紐づくユニークなサブディレクトリを作成
    job_media_dir = os.path.join(SCHEDULED_MEDIA_DIR, str(uuid.uuid4()))
    os.makedirs(job_media_dir, exist_ok=True)

    media_paths = []
    try:
        for file in media_files:
            path = os.path.join(job_media_dir, file.filename)
            with open(path, "wb") as buffer:
                shutil.copyfileobj(file.file, buffer)
            media_paths.append(path)

        post_data = {
            'text': text,
            'url': url,
            'sns_targets': sns_targets,
            'media_files': media_paths,
            'job_media_dir': job_media_dir # 投稿後に削除するためにディレクトリパスも渡す
        }

        run_date = datetime.fromisoformat(schedule_time)

        # ジョブIDを生成し、後でメディアファイルを削除するために使用
        job_id = str(uuid.uuid4())
        scheduler_service.scheduler.add_job(
            posting_service.post_now_and_cleanup, # 新しいクリーンアップ付きメソッドを呼び出す
            'date',
            run_date=run_date,
            args=[post_data],
            id=job_id,
            misfire_grace_time=600 # 10分間の猶予
        )
        
        return JSONResponse(content={"message": "Post scheduled successfully", "job_id": job_id})

    except Exception as e:
        # エラー発生時は作成したディレクトリをクリーンアップ
        if os.path.exists(job_media_dir):
            shutil.rmtree(job_media_dir)
        raise HTTPException(status_code=400, detail=str(e))
