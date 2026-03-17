"""即時投稿関連ルート"""
import logging
import os
import shutil
import tempfile
from concurrent.futures import ThreadPoolExecutor
from typing import List

from fastapi import APIRouter, Depends, File, Form, HTTPException, Request, UploadFile
from fastapi.responses import JSONResponse

from ...config_manager import ConfigManager
from ..dependencies import (
    get_config_manager,
    get_current_user,
    get_executor,
    get_posting_service,
    get_scheduled_post_store,
    get_ticket_manager,
    get_valid_sns_names,
)
from ..posting_service import PostingService
from ..scheduled_post_store_sqlite import ScheduledPostStoreSQLite
from ..ticket_manager import TicketManager
from ..upload_validator import validate_upload_files

router = APIRouter(prefix="/api")
logger = logging.getLogger(__name__)


@router.post("/post")
def api_post(
    request: Request,
    text: str = Form(...),
    url: str = Form(None),
    sns_targets: List[str] = Form(...),
    media_files: List[UploadFile] = File([]),
    user: str = Depends(get_current_user),
    config_manager: ConfigManager = Depends(get_config_manager),
    posting_service: PostingService = Depends(get_posting_service),
    ticket_manager: TicketManager = Depends(get_ticket_manager),
    executor: ThreadPoolExecutor = Depends(get_executor)
):
    """チケットベースの即時投稿API

    複数SNSへの投稿をバックグラウンドで並列実行し、チケットIDを返す。
    各SNSの投稿状態は /api/post_status/{ticket_id}/{sns} で取得可能。
    """
    # SNS名の検証
    valid_sns_names = get_valid_sns_names(config_manager)
    invalid_sns = [sns for sns in sns_targets if sns not in valid_sns_names]
    if invalid_sns:
        return JSONResponse(
            status_code=400,
            content={"error": f"無効なSNS名: {', '.join(invalid_sns)}"}
        )

    # テキストが空でないかチェック
    if not text or not text.strip():
        return JSONResponse(
            status_code=400,
            content={"error": "投稿テキストは空にできません"}
        )

    # メディアファイルのバリデーション
    if media_files:
        upload_error = validate_upload_files(media_files)
        if upload_error:
            return JSONResponse(status_code=400, content={"error": upload_error})

    # メディアファイル保存（TicketManagerが削除を管理）
    media_paths = []
    if media_files:
        temp_dir = tempfile.mkdtemp()
        for file in media_files:
            if not file.filename:
                continue
            # Path Traversal対策: ファイル名をサニタイズ
            safe_filename = os.path.basename(file.filename)
            path = os.path.join(temp_dir, safe_filename)
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


@router.get("/post_status/{ticket_id}/{sns}")
def get_post_status(
    ticket_id: str,
    sns: str,
    user: str = Depends(get_current_user),
    config_manager: ConfigManager = Depends(get_config_manager),
    ticket_manager: TicketManager = Depends(get_ticket_manager)
):
    """特定SNSの投稿状態を取得

    Args:
        ticket_id: チケットID
        sns: SNS名（x, bluesky, mastodon, misskey, threads, tumblr）

    Returns:
        {'status': 'processing' | 'success' | 'failed' | 'error', 'message': str}
    """
    # SNS名の検証
    valid_sns_names = get_valid_sns_names(config_manager)
    if sns not in valid_sns_names:
        raise HTTPException(
            status_code=400,
            detail=f"無効なSNS名: {sns}"
        )

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


@router.post("/post-history")
async def save_post_history(
    request: Request,
    user: str = Depends(get_current_user),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store)
):
    """即時投稿の履歴を保存

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

    from ..scheduled_post_model import ScheduledPost
    from ..timezone_utils import now_local

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
        store.create_post(history_post)
        logger.info(f"Post history saved: {history_post.id}")

        return JSONResponse(content={
            "success": True,
            "id": history_post.id,
            "message": "投稿履歴が保存されました"
        }, status_code=201)

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error saving post history: {str(e)}")
        raise HTTPException(status_code=400, detail=f"Failed to save post history: {str(e)}")
