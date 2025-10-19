"""FastAPI依存性注入用モジュール

各サービスのインスタンスをグローバル変数として管理し、
Depends()を通じて各エンドポイントに注入する。
"""
import logging
import os
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Optional

from fastapi import Depends, HTTPException, Request, status
from fastapi.templating import Jinja2Templates

from ..config_manager import ConfigManager
from ..image_resizer import ImageResizer
from ..text_optimizer import TextOptimizer
from .auth_service import AuthService
from .post_executor import PostExecutor
from .posting_service import PostingService
from .scheduled_post_store_sqlite import ScheduledPostStoreSQLite
from .scheduler_service import SchedulerService
from .ticket_manager import TicketManager

logger = logging.getLogger(__name__)

# データディレクトリの確認と作成
DATA_DIR = os.environ.get("DATA_DIR", "data")
if not os.path.exists(DATA_DIR):
    os.makedirs(DATA_DIR)

# 予約投稿データストアのパス（SQLite ベース）
SCHEDULED_POSTS_DB = str(Path(DATA_DIR) / "scheduled_posts.db")

# グローバルインスタンス（起動時初期化）
_scheduled_post_store: Optional[ScheduledPostStoreSQLite] = None
_config_manager: Optional[ConfigManager] = None
_auth_service: Optional[AuthService] = None
_post_executor: Optional[PostExecutor] = None
_scheduler_service: Optional[SchedulerService] = None
_image_resizer: Optional[ImageResizer] = None
_text_optimizer: Optional[TextOptimizer] = None
_posting_service: Optional[PostingService] = None
_ticket_manager: Optional[TicketManager] = None
_executor: Optional[ThreadPoolExecutor] = None
_templates: Optional[Jinja2Templates] = None


def initialize_services(config_path: str = "config.yml"):
    """全サービスを初期化（アプリ起動時に一度だけ呼ぶ）"""
    global _scheduled_post_store, _config_manager, _auth_service
    global _post_executor, _scheduler_service, _image_resizer
    global _text_optimizer, _posting_service, _ticket_manager
    global _executor, _templates

    # ストレージ初期化
    _scheduled_post_store = ScheduledPostStoreSQLite(SCHEDULED_POSTS_DB)

    # 設定と認証
    _config_manager = ConfigManager(config_path)
    _auth_service = AuthService(_config_manager)

    # 投稿実行とスケジューラー
    _post_executor = PostExecutor(_scheduled_post_store, _config_manager)
    retention_hours = _config_manager.get_completed_post_retention_hours()
    _scheduler_service = SchedulerService(
        _scheduled_post_store,
        _post_executor,
        data_dir=DATA_DIR,
        completed_post_retention_hours=retention_hours
    )

    # 投稿関連サービス
    _image_resizer = ImageResizer()
    _text_optimizer = TextOptimizer(_config_manager.config)
    _posting_service = PostingService(
        config_manager=_config_manager,
        image_resizer=_image_resizer,
        text_optimizer=_text_optimizer
    )

    # チケット管理とスレッドプール
    _ticket_manager = TicketManager(ticket_lifetime_hours=24)
    _executor = ThreadPoolExecutor(max_workers=5)

    # テンプレート
    _templates = Jinja2Templates(directory="src/web/templates")


def get_scheduled_post_store() -> ScheduledPostStoreSQLite:
    """予約投稿ストアを取得"""
    if _scheduled_post_store is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _scheduled_post_store


def get_config_manager() -> ConfigManager:
    """設定マネージャーを取得"""
    if _config_manager is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")

    # テスト用モック対応
    manager = _config_manager
    if callable(manager) and not isinstance(manager, ConfigManager):
        try:
            resolved = manager()
        except TypeError:
            resolved = None
        if resolved is not None:
            manager = resolved
    return manager


def get_auth_service() -> AuthService:
    """認証サービスを取得"""
    if _auth_service is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _auth_service


def get_post_executor() -> PostExecutor:
    """投稿実行サービスを取得"""
    if _post_executor is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _post_executor


def get_scheduler_service() -> SchedulerService:
    """スケジューラーサービスを取得"""
    if _scheduler_service is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _scheduler_service


def get_image_resizer() -> ImageResizer:
    """画像リサイザーを取得"""
    if _image_resizer is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _image_resizer


def get_text_optimizer() -> TextOptimizer:
    """テキスト最適化サービスを取得"""
    if _text_optimizer is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _text_optimizer


def get_posting_service() -> PostingService:
    """投稿サービスを取得"""
    if _posting_service is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _posting_service


def get_ticket_manager() -> TicketManager:
    """チケット管理サービスを取得"""
    if _ticket_manager is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _ticket_manager


def get_executor() -> ThreadPoolExecutor:
    """スレッドプール実行サービスを取得"""
    if _executor is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _executor


def get_templates() -> Jinja2Templates:
    """テンプレートエンジンを取得"""
    if _templates is None:
        raise RuntimeError("Services not initialized. Call initialize_services() first.")
    return _templates


def get_data_dir() -> str:
    """データディレクトリパスを取得"""
    return DATA_DIR


def get_csrf_token(request: Request) -> str:
    """CSRFトークンを取得

    CSRFCookieMiddleware stores the token on request.state.
    Fallback to cookies or session just in case.
    """
    token = getattr(request.state, "csrf_token", None)
    if token:
        return token
    token = request.cookies.get("csrftoken")
    if token:
        return token
    return request.session.get("csrf_token", "")


def get_current_user(request: Request) -> str:
    """認証済みユーザーを取得（認証チェック付き）"""
    user = request.session.get("user")
    if not user:
        raise HTTPException(
            status_code=status.HTTP_303_SEE_OTHER,
            detail="Not authenticated",
            headers={"Location": "/login"},
        )
    return user


def get_valid_sns_names(
    config_manager: ConfigManager = Depends(get_config_manager)
) -> set[str]:
    """有効なSNS名一覧を取得

    設定ファイルに登録された全SNS識別子を返す。
    アカウント名とプロバイダータイプの両方を含む。
    """
    try:
        sns_configs = config_manager.get_all_sns_configs()
    except AttributeError:
        sns_configs = None

    valid_names: set[str] = set()

    if isinstance(sns_configs, list):
        for config in sns_configs:
            sns_type = config.get('type')
            sns_name = config.get('name') or sns_type
            if sns_type:
                valid_names.add(sns_type)
            if sns_name:
                valid_names.add(sns_name)
    elif isinstance(sns_configs, dict):
        for name, config in sns_configs.items():
            if name:
                valid_names.add(name)
            sns_type = config.get('type')
            if sns_type:
                valid_names.add(sns_type)
    else:
        try:
            sns_names = config_manager.get_all_sns_names()
        except AttributeError:
            sns_names = None
        if isinstance(sns_names, (list, set, tuple)):
            valid_names.update(sns_names)

    return valid_names
