"""トップページルート"""
from typing import Optional

from fastapi import APIRouter, Depends, Request
from fastapi.templating import Jinja2Templates

from ...config_manager import ConfigManager
from ..dependencies import (
    get_config_manager,
    get_csrf_token,
    get_current_user,
    get_scheduled_post_store,
    get_templates,
)
from ..scheduled_post_store_sqlite import ScheduledPostStoreSQLite
from ..timezone_utils import ensure_local_timezone, now_local

router = APIRouter()


@router.get("/")
def read_root(
    request: Request,
    sort_by: Optional[str] = 'date_asc',
    user: str = Depends(get_current_user),
    config_manager: ConfigManager = Depends(get_config_manager),
    store: ScheduledPostStoreSQLite = Depends(get_scheduled_post_store),
    templates: Jinja2Templates = Depends(get_templates)
):
    """トップページ（投稿管理画面）"""
    sns_configs = config_manager.get_all_sns_configs()
    sns_accounts = []
    if isinstance(sns_configs, list):
        for config in sns_configs:
            sns_accounts.append({'name': config.get('name'), 'type': config.get('type')})
    elif isinstance(sns_configs, dict):
        for name, config in sns_configs.items():
            sns_type = config.get('type', name)
            sns_accounts.append({'name': name, 'type': sns_type})

    scheduled_posts = store.get_all_posts(sort_by=sort_by)
    for post in scheduled_posts:
        scheduled_at_tz = ensure_local_timezone(post.scheduled_at)
        if scheduled_at_tz is not None:
            post.scheduled_at = scheduled_at_tz

    return templates.TemplateResponse(
        "index.html",
        {
            "request": request,
            "user": user,
            "sns_accounts": sns_accounts,
            "scheduled_posts": scheduled_posts,
            "now": now_local(),
            "current_sort_by": sort_by,
            "csrf_token": get_csrf_token(request),
        },
    )
