"""予約投稿管理ルート"""
import logging
import os
import shutil
import uuid
from datetime import datetime, timedelta
from typing import List, Optional

from fastapi import APIRouter, Depends, File, Form, HTTPException, UploadFile, status
from fastapi.responses import JSONResponse

from ...config_manager import ConfigManager
from ..dependencies import (
    get_config_manager,
    get_current_user,
    get_data_dir,
    get_post_executor,
    get_posting_service,
    get_scheduled_post_store,
    get_scheduler_service,
)
from ..post_executor import PostExecutor
from ..posting_service import PostingService
from ..scheduled_post_model import ScheduledPost
from ..scheduled_post_store_sqlite import ScheduledPostStoreSQLite
from ..scheduler_service import SchedulerService
from ..timezone_utils import ensure_local_timezone, now_local

router = APIRouter(prefix="/api")
logger = logging.getLogger(__name__)


@router.get("/posts", response_model=List[ScheduledPost])
def get_api_posts(
    sort_by: Optional[str] = 'date_asc',
    user: str = Depends(get_current_user),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store)
):
    """全投稿一覧取得（ソート指定可能）"""
    return store.get_all_posts(sort_by=sort_by)


@router.get("/scheduled-posts", response_model=List[ScheduledPost])
def get_all_scheduled_posts(
    user: str = Depends(get_current_user),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store)
):
    """全予約投稿一覧取得"""
    return store.get_all_posts()


@router.get("/scheduled-posts/{post_id}", response_model=ScheduledPost)
def get_scheduled_post(
    post_id: str,
    user: str = Depends(get_current_user),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store)
):
    """特定の予約投稿取得"""
    post = store.get_post_by_id(post_id)
    if not post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")
    return post


@router.post("/scheduled-posts", response_model=ScheduledPost, status_code=status.HTTP_201_CREATED)
def create_scheduled_post(
    scheduled_at: datetime = Form(...),
    content: str = Form(...),
    media_files: List[UploadFile] = File([]),
    target_sns: List[str] = Form(...),
    user: str = Depends(get_current_user),
    config_manager: ConfigManager = Depends(get_config_manager),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store),
    data_dir: str = Depends(get_data_dir)
):
    """予約投稿作成"""
    media_paths = []
    if media_files:
        post_media_dir = os.path.join(data_dir, "scheduled_media", str(uuid.uuid4()))
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
    store.create_post(new_post)
    return new_post


@router.put("/scheduled-posts/{post_id}", response_model=ScheduledPost)
def update_scheduled_post(
    post_id: str,
    scheduled_at: Optional[datetime] = Form(None),
    content: Optional[str] = Form(None),
    media_files: List[UploadFile] = File([]),
    target_sns: Optional[List[str]] = Form(None),
    user: str = Depends(get_current_user),
    config_manager: ConfigManager = Depends(get_config_manager),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store)
):
    """予約投稿更新"""
    existing_post = store.get_post_by_id(post_id)
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

    updated_post = store.update_post(post_id, updates)
    if not updated_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found after update attempt")
    return updated_post


@router.delete("/scheduled-posts/{post_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_scheduled_post(
    post_id: str,
    user: str = Depends(get_current_user),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store)
):
    """予約投稿削除"""
    existing_post = store.get_post_by_id(post_id)
    if not existing_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")

    if existing_post.status == "実行済み":
        raise HTTPException(status_code=409, detail="Cannot delete an already executed post")

    if not store.delete_post(post_id):
        raise HTTPException(status_code=404, detail="Scheduled post not found for deletion")
    return


@router.post("/scheduled-posts/batch-delete")
def batch_delete_scheduled_posts(
    post_ids: List[str] = Form(...),
    user: str = Depends(get_current_user),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store)
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

    deleted_count = store.batch_delete_posts(post_ids)

    logger.info(f"User {user} batch deleted {deleted_count} posts")

    return {"deleted_count": deleted_count}


@router.post("/scheduled-posts/{post_id}/re-execute", response_model=ScheduledPost)
def re_execute_scheduled_post(
    post_id: str,
    user: str = Depends(get_current_user),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store)
):
    """失敗した投稿を再実行（1分後に再スケジュール）"""
    existing_post = store.get_post_by_id(post_id)
    if not existing_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found")

    if existing_post.status == "実行済み":
        raise HTTPException(status_code=409, detail="Cannot re-execute an already successful post")

    updates = {
        "scheduled_at": now_local() + timedelta(minutes=1),
        "status": "予約済み",
        "error_message": None
    }
    updated_post = store.update_post(post_id, updates)
    if not updated_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found for re-execution")

    return updated_post


@router.post("/scheduled-posts/{post_id}/send-now", response_model=ScheduledPost)
def send_scheduled_post_now(
    post_id: str,
    user: str = Depends(get_current_user),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store),
    post_executor: PostExecutor = Depends(get_post_executor)
):
    """予約投稿を即座に実行"""
    existing_post = store.get_post_by_id(post_id)
    if not existing_post:
        logger.warning(f"User {user} attempted to send non-existent post {post_id}")
        raise HTTPException(status_code=404, detail="Scheduled post not found")

    if existing_post.status == "実行済み":
        logger.warning(f"User {user} attempted to send already executed post {post_id}")
        raise HTTPException(status_code=409, detail="Cannot send an already executed post immediately")

    logger.info(f"User {user} requested immediate sending of post {post_id}")
    success = post_executor.execute_post(post_id, debug=True)

    updated_post = store.get_post_by_id(post_id)
    if not updated_post:
        raise HTTPException(status_code=404, detail="Scheduled post not found after immediate sending")

    if success:
        logger.info(f"Post {post_id} sent immediately successfully by user {user}")
    else:
        logger.error(f"Post {post_id} failed to send immediately for user {user}")

    return updated_post


@router.post("/scheduled-posts/next", response_model=dict)
def schedule_post_next_timing(
    content: str = Form(...),
    target_sns: List[str] = Form(...),
    media_files: List[UploadFile] = File([]),
    user: str = Depends(get_current_user),
    config_manager: ConfigManager = Depends(get_config_manager),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store),
    data_dir: str = Depends(get_data_dir)
):
    """次のタイミングで投稿（各SNSの次の空きスロットに自動予約）
    
    Returns:
        {
            "created_posts": [
                {"id": str, "sns": str, "scheduled_at": datetime, "status": str}
            ],
            "errors": [
                {"sns": str, "error": str}
            ]
        }
    """
    # メディアファイル処理
    media_paths = []
    if media_files:
        post_media_dir = os.path.join(data_dir, "scheduled_media", str(uuid.uuid4()))
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

    # SNS検証
    supported_sns = config_manager.get_all_sns_names()
    if not all(sns in supported_sns for sns in target_sns):
        raise HTTPException(status_code=400, detail="Unsupported SNS target specified")

    logger.info(f"User {user} requesting next timing posts for {len(target_sns)} SNS")

    # スロット検索
    from ..slot_finder import SlotFinder
    from ...timing_manager import TimingManager
    
    timing_manager = TimingManager(config_manager)
    slot_finder = SlotFinder(timing_manager, store)
    
    slots = slot_finder.find_slots_for_multiple_sns(target_sns)
    
    created_posts = []
    errors = []
    
    for sns in target_sns:
        slot_time = slots.get(sns)
        
        if slot_time is None:
            errors.append({
                "sns": sns,
                "error": "7日以内に空きスロットが見つかりませんでした"
            })
            logger.warning(f"No available slot found for SNS {sns} within 7 days")
            continue
        
        try:
            # 予約投稿を作成
            new_post = ScheduledPost(
                scheduled_at=slot_time,
                content=content,
                media_files=media_paths,
                target_sns=[sns]
            )
            store.create_post(new_post)
            
            created_posts.append({
                "id": new_post.id,
                "sns": sns,
                "scheduled_at": new_post.scheduled_at,
                "status": new_post.status
            })
            
            logger.info(f"Created scheduled post for {sns} at {slot_time}")
            
        except Exception as e:
            errors.append({
                "sns": sns,
                "error": f"予約作成に失敗しました: {str(e)}"
            })
            logger.error(f"Failed to create post for {sns}: {e}")
    
    logger.info(f"User {user} created {len(created_posts)} posts with {len(errors)} errors")
    
    return {
        "created_posts": created_posts,
        "errors": errors
    }


@router.post("/schedule")
def api_schedule(
    text: str = Form(...),
    url: str = Form(None),
    sns_targets: List[str] = Form(...),
    media_files: List[UploadFile] = File([]),
    schedule_time: str = Form(...),
    user: str = Depends(get_current_user),
    posting_service: PostingService = Depends(get_posting_service),
    scheduler_service: SchedulerService = Depends(get_scheduler_service),
    data_dir: str = Depends(get_data_dir)
):
    """投稿をスケジュール（旧API互換）"""
    SCHEDULED_MEDIA_DIR = os.path.join(data_dir, "scheduled_media")
    os.makedirs(SCHEDULED_MEDIA_DIR, exist_ok=True)

    job_media_dir = os.path.join(SCHEDULED_MEDIA_DIR, str(uuid.uuid4()))
    os.makedirs(job_media_dir, exist_ok=True)

    media_paths = []
    try:
        for file in media_files:
            if not file.filename:
                continue
            # Path Traversal対策: ファイル名をサニタイズ
            safe_filename = os.path.basename(file.filename)
            path = os.path.join(job_media_dir, safe_filename)
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
