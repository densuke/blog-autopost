import logging
import os
import shutil
import tempfile
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta
from pathlib import Path
from typing import List, Optional

from fastapi import (
    Depends,
    FastAPI,
    File,
    Form,
    HTTPException,
    Request,
    UploadFile,
    status,
)
from fastapi.responses import JSONResponse, RedirectResponse
from fastapi.templating import Jinja2Templates
from starlette.middleware.sessions import SessionMiddleware

from ..config_manager import ConfigManager
from ..image_resizer import ImageResizer
from ..text_optimizer import TextOptimizer
from .auth_service import AuthService
from .post_executor import PostExecutor
from .posting_service import PostingService
from .scheduled_post_model import ScheduledPost
from .scheduled_post_store_sqlite import ScheduledPostStoreSQLite
from .scheduler_service import SchedulerService
from .ticket_manager import TicketManager
from .timezone_utils import ensure_local_timezone, now_local

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

# 予約投稿データストアのパス（SQLite ベース）
SCHEDULED_POSTS_FILE = Path(DATA_DIR) / "scheduled_posts.json"
SCHEDULED_POSTS_DB = str(Path(DATA_DIR) / "scheduled_posts.db")
# SQLite ストアを使用（自動的に JSONからのマイグレーションも実行）
scheduled_post_store = ScheduledPostStoreSQLite(SCHEDULED_POSTS_DB)

# 設定と認証サービスのインスタンス化
config_manager = ConfigManager("config.yml")
auth_service = AuthService(config_manager)

# 投稿実行サービスとスケジューラーサービスのインスタンス化
post_executor = PostExecutor(scheduled_post_store, config_manager)
retention_hours = config_manager.get_completed_post_retention_hours()
scheduler_service = SchedulerService(scheduled_post_store, post_executor, data_dir=DATA_DIR, completed_post_retention_hours=retention_hours)

@app.on_event("startup")
def startup_event():
    scheduler_service.start()

@app.on_event("shutdown")
def shutdown_event():
    scheduler_service.shutdown()


# 投稿関連サービスのインスタンス化
image_resizer = ImageResizer()
text_optimizer = TextOptimizer(config_manager.config)
posting_service = PostingService(
    config_manager=config_manager,
    image_resizer=image_resizer,
    text_optimizer=text_optimizer
)

# チケット管理システムとスレッドプールのインスタンス化
ticket_manager = TicketManager(ticket_lifetime_hours=24)
executor = ThreadPoolExecutor(max_workers=5)

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
def read_root(request: Request, sort_by: Optional[str] = 'date_asc', user: str = Depends(get_current_user)):
    sns_configs = config_manager.get_all_sns_configs()
    sns_accounts = []
    if isinstance(sns_configs, list):
        for config in sns_configs:
            sns_accounts.append({'name': config.get('name'), 'type': config.get('type')})
    elif isinstance(sns_configs, dict):
        for name, config in sns_configs.items():
            sns_type = config.get('type', name)
            sns_accounts.append({'name': name, 'type': sns_type})

    scheduled_posts = scheduled_post_store.get_all_posts(sort_by=sort_by)
    for post in scheduled_posts:
        scheduled_at_tz = ensure_local_timezone(post.scheduled_at)
        if scheduled_at_tz is not None:
            post.scheduled_at = scheduled_at_tz

    return templates.TemplateResponse("index.html", {"request": request, "user": user, "sns_accounts": sns_accounts, "scheduled_posts": scheduled_posts, "now": now_local(), "current_sort_by": sort_by})

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
    """チケットベースの即時投稿API

    複数SNSへの投稿をバックグラウンドで並列実行し、チケットIDを返す。
    各SNSの投稿状態は /api/post_status/{ticket_id}/{sns} で取得可能。
    """
    # メディアファイル保存（TicketManagerが削除を管理）
    media_paths = []
    if media_files:
        temp_dir = tempfile.mkdtemp()
        for file in media_files:
            if not file.filename:
                continue
            path = os.path.join(temp_dir, file.filename)
            with open(path, "wb") as buffer:
                shutil.copyfileobj(file.file, buffer)
            media_paths.append(path)

    # チケット発行
    ticket_id = ticket_manager.create_ticket(sns_targets, media_paths)
    logger.info(f"User {user} created ticket {ticket_id} for SNS targets: {sns_targets}")

    # post_dataの準備
    post_data = {
        'text': text,
        'url': url,
        'sns_targets': sns_targets,
        'media_files': media_paths
    }

    # バックグラウンドで各SNSへ投稿
    def post_to_sns_background(sns_name: str):
        """個別SNSへの投稿（バックグラウンド実行）"""
        try:
            # 単一SNS向けのpost_dataを作成
            single_sns_data = post_data.copy()
            single_sns_data['sns_targets'] = [sns_name]

            # 投稿実行
            result = posting_service.post_now(single_sns_data, debug=False)

            # 結果を取得
            sns_result = result.get(sns_name, {'success': False, 'message': 'No result returned'})

            if sns_result.get('success'):
                ticket_manager.update_status(ticket_id, sns_name, 'success', sns_result.get('message'))
            else:
                ticket_manager.update_status(ticket_id, sns_name, 'failed', sns_result.get('message'))

        except Exception as e:
            logger.error(f"Error posting to {sns_name} for ticket {ticket_id}: {str(e)}")
            ticket_manager.update_status(ticket_id, sns_name, 'error', str(e))

    # スレッドプールで各SNSへ並列投稿
    for sns_name in sns_targets:
        executor.submit(post_to_sns_background, sns_name)

    # チケットIDを即座に返す
    return JSONResponse(content={
        "ticket_id": ticket_id,
        "status": "processing",
        "message": "投稿処理を開始しました"
    })

@app.get("/api/post_status/{ticket_id}/{sns}")
def get_post_status(
    ticket_id: str,
    sns: str,
    user: str = Depends(get_current_user)
):
    """特定SNSの投稿状態を取得

    Args:
        ticket_id: チケットID
        sns: SNS名（x, bluesky, mastodon, misskey）

    Returns:
        {'status': 'processing' | 'success' | 'failed' | 'error', 'message': str}
    """
    status_info = ticket_manager.get_status(ticket_id, sns)

    if status_info is None:
        raise HTTPException(
            status_code=404,
            detail=f"Ticket {ticket_id} not found or expired for SNS {sns}"
        )

    return JSONResponse(content={
        'sns': sns,
        'status': status_info['status'],
        'message': status_info.get('message'),
        'updated_at': status_info['updated_at'].isoformat()
    })

@app.get("/api/posts", response_model=List[ScheduledPost])
def get_api_posts(sort_by: Optional[str] = 'date_asc', user: str = Depends(get_current_user)):
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
    media_paths = []
    if media_files:
        import uuid
        post_media_dir = os.path.join(DATA_DIR, "scheduled_media", str(uuid.uuid4()))
        os.makedirs(post_media_dir, exist_ok=True)
        os.chmod(post_media_dir, 0o700)

        for file in media_files:
            if not file.filename:
                continue
            safe_filename = os.path.basename(file.filename)
            path = os.path.join(post_media_dir, safe_filename)

            with open(path, "wb") as buffer:
                shutil.copyfileobj(file.file, buffer)

            os.chmod(path, 0o600)
            media_paths.append(path)

    supported_sns = config_manager.get_all_sns_names()
    if not all(sns in supported_sns for sns in target_sns):
        raise HTTPException(status_code=400, detail="Unsupported SNS target specified")

    scheduled_at_tz = ensure_local_timezone(scheduled_at)
    if scheduled_at_tz is None:
        scheduled_at_tz = scheduled_at
    logger.info(f"User {user} creating scheduled post for {scheduled_at_tz}")
    new_post = ScheduledPost(
        scheduled_at=scheduled_at_tz,
        content=content,
        media_files=media_paths,
        target_sns=target_sns
    )
    scheduled_post_store.create_post(new_post)
    return new_post

@app.put("/api/scheduled-posts/{post_id}", response_model=ScheduledPost)
def update_scheduled_post(
    post_id: str,
    scheduled_at: Optional[datetime] = Form(None),
    content: Optional[str] = Form(None),
    media_files: List[UploadFile] = File([]),
    target_sns: Optional[List[str]] = Form(None),
    user: str = Depends(get_current_user)
):
    existing_post = scheduled_post_store.get_post_by_id(post_id)
    if not existing_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")

    if existing_post.status in ["実行済み", "失敗"]:
        raise HTTPException(status_code=409, detail="Cannot update an already executed or failed post")

    updates: dict[str, datetime | str | list[str]] = {}
    if scheduled_at:
        scheduled_at_tz = ensure_local_timezone(scheduled_at)
        updates["scheduled_at"] = scheduled_at_tz if scheduled_at_tz is not None else scheduled_at
    if content:
        updates["content"] = content
    if target_sns is not None:
        supported_sns = config_manager.get_all_sns_names()
        if not all(sns in supported_sns for sns in target_sns):
            raise HTTPException(status_code=400, detail="Unsupported SNS target specified")
        updates["target_sns"] = target_sns

    updated_post = scheduled_post_store.update_post(post_id, updates)
    if not updated_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found after update attempt")
    return updated_post

@app.delete("/api/scheduled-posts/{post_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_scheduled_post(post_id: str, user: str = Depends(get_current_user)):
    existing_post = scheduled_post_store.get_post_by_id(post_id)
    if not existing_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")

    if existing_post.status == "実行済み":
        raise HTTPException(status_code=409, detail="Cannot delete an already executed post")

    if not scheduled_post_store.delete_post(post_id):
        raise HTTPException(status_code=404, detail="Scheduled post not found for deletion")
    return


@app.post("/api/scheduled-posts/batch-delete")
def batch_delete_scheduled_posts(
    post_ids: List[str] = Form(...),
    user: str = Depends(get_current_user)
):
    """複数の予約投稿を一括削除
    
    Args:
        post_ids: 削除対象の投稿ID リスト（Form パラメータ）
        user: 認証済みユーザー
    
    Returns:
        { "deleted_count": 削除件数 }
    """
    if not post_ids:
        raise HTTPException(status_code=400, detail="No post IDs provided")

    logger.info(f"User {user} requesting batch delete for {len(post_ids)} posts")

    deleted_count = scheduled_post_store.batch_delete_posts(post_ids)

    logger.info(f"User {user} batch deleted {deleted_count} posts")

    return {"deleted_count": deleted_count}

@app.post("/api/scheduled-posts/{post_id}/re-execute", response_model=ScheduledPost)
def re_execute_scheduled_post(post_id: str, user: str = Depends(get_current_user)):
    existing_post = scheduled_post_store.get_post_by_id(post_id)
    if not existing_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")

    if existing_post.status == "実行済み":
        raise HTTPException(status_code=409, detail="Cannot re-execute an already successful post")

    updates = {
        "scheduled_at": now_local() + timedelta(minutes=1),
        "status": "予約済み",
        "error_message": None
    }
    updated_post = scheduled_post_store.update_post(post_id, updates)
    if not updated_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found for re-execution")

    return updated_post

@app.post("/api/scheduled-posts/{post_id}/send-now", response_model=ScheduledPost)
def send_scheduled_post_now(post_id: str, user: str = Depends(get_current_user)):
    existing_post = scheduled_post_store.get_post_by_id(post_id)
    if not existing_post:
        logger.warning(f"User {user} attempted to send non-existent post {post_id}")
        raise HTTPException(status_code=404, detail="Scheduled post not found")

    if existing_post.status == "実行済み":
        logger.warning(f"User {user} attempted to send already executed post {post_id}")
        raise HTTPException(status_code=409, detail="Cannot send an already executed post immediately")

    logger.info(f"User {user} requested immediate sending of post {post_id}")
    success = post_executor.execute_post(post_id, debug=True)

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
    import uuid
    SCHEDULED_MEDIA_DIR = os.path.join(DATA_DIR, "scheduled_media")
    os.makedirs(SCHEDULED_MEDIA_DIR, exist_ok=True)

    job_media_dir = os.path.join(SCHEDULED_MEDIA_DIR, str(uuid.uuid4()))
    os.makedirs(job_media_dir, exist_ok=True)

    media_paths = []
    try:
        for file in media_files:
            if not file.filename:
                continue
            path = os.path.join(job_media_dir, file.filename)
            with open(path, "wb") as buffer:
                shutil.copyfileobj(file.file, buffer)
            media_paths.append(path)

        post_data = {
            'text': text,
            'url': url,
            'sns_targets': sns_targets,
            'media_files': media_paths,
            'job_media_dir': job_media_dir
        }

        run_date = ensure_local_timezone(datetime.fromisoformat(schedule_time))

        job_id = str(uuid.uuid4())
        scheduler_service.scheduler.add_job(
            posting_service.post_now_and_cleanup,
            'date',
            run_date=run_date,
            args=[post_data],
            id=job_id,
            misfire_grace_time=600
        )

        return JSONResponse(content={"message": "Post scheduled successfully", "job_id": job_id})

    except Exception as e:
        if os.path.exists(job_media_dir):
            shutil.rmtree(job_media_dir)
        raise HTTPException(status_code=400, detail=str(e))


@app.post("/api/post-history")
async def save_post_history(
    request: Request,
    user: str = Depends(get_current_user)
):
    """
    即時投稿の履歴を保存

    即時投稿後に、投稿内容をデータベースに記録するエンドポイント。
    予約投稿ではなく、既に実行された投稿の履歴を保存する。

    リクエストボディ例:
    {
        "content": "投稿文",
        "target_sns": ["x", "bluesky"],
        "status": "投稿完了",
        "error_message": null,
        "scheduled_at": "2025-10-17T10:30:00.000Z"
    }
    """
    try:
        # リクエストボディをJSONで解析
        body = await request.json()

        content = body.get('content', '')
        target_sns = body.get('target_sns', [])
        status_val = body.get('status', '投稿完了')
        error_message = body.get('error_message')

        if not content:
            raise HTTPException(status_code=400, detail="content is required")

        if not target_sns:
            raise HTTPException(status_code=400, detail="target_sns is required")

        logger.info(f"User {user} saving post history: content={content[:50]}..., target_sns={target_sns}")

        # 現在時刻をJSTで設定
        now = now_local()

        # 投稿履歴をスケジュール済み投稿として保存（status='投稿完了'）
        history_post = ScheduledPost(
            scheduled_at=now,
            content=content,
            target_sns=target_sns,
            status=status_val,
            error_message=error_message,
            media_files=[]  # 即時投稿履歴はメディアファイル参照不要
        )

        # データベースに保存
        scheduled_post_store.create_post(history_post)
        logger.info(f"Post history saved: {history_post.id}")

        return JSONResponse(content={
            "success": True,
            "id": history_post.id,
            "message": "投稿履歴が保存されました"
        }, status_code=status.HTTP_201_CREATED)

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error saving post history: {str(e)}")
        raise HTTPException(status_code=400, detail=f"Failed to save post history: {str(e)}")
