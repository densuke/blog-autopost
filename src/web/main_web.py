"""Blog AutoPost Web Application

FastAPIベースのWebアプリケーション。
SNS投稿管理（即時投稿・予約投稿）を行う。
"""
import logging
from contextlib import asynccontextmanager

from fastapi import FastAPI
from slowapi import _rate_limit_exceeded_handler
from slowapi.errors import RateLimitExceeded
from starlette.middleware.sessions import SessionMiddleware

from .csrf_protection import CSRFCookieMiddleware, FormCSRFMiddleware
from .dependencies import get_config_manager, get_csrf_secret_key, get_scheduler_service, initialize_services
from .rate_limiter import limiter
from .routes import auth, index, posts, scheduled_posts
from .security_headers import SecurityHeadersMiddleware

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

# サービス初期化
initialize_services("config.yml")


@asynccontextmanager
async def lifespan(app: FastAPI):
    """アプリケーションのライフサイクル管理（起動・終了処理）"""
    scheduler_service = get_scheduler_service()
    scheduler_service.start()
    yield
    scheduler_service.shutdown()


# アプリケーション作成
app = FastAPI(lifespan=lifespan)

# レート制限の設定
app.state.limiter = limiter
app.add_exception_handler(RateLimitExceeded, _rate_limit_exceeded_handler)  # type: ignore[arg-type]

# ミドルウェア設定
config_manager = get_config_manager()
secret_key = config_manager.get_secret_key()
if not secret_key:
    raise RuntimeError("セッション管理用のsecret_keyが設定されていません。config.ymlを確認してください。")

csrf_secret = config_manager.get_csrf_secret_key() or secret_key
cookie_secure = config_manager.get_cookie_secure()

app.add_middleware(SecurityHeadersMiddleware)
app.add_middleware(SessionMiddleware, secret_key=secret_key, https_only=cookie_secure)
app.add_middleware(CSRFCookieMiddleware, secret=csrf_secret, cookie_secure=cookie_secure)
app.add_middleware(FormCSRFMiddleware, secret=csrf_secret)

# ルート登録
app.include_router(index.router)
app.include_router(auth.router)
app.include_router(posts.router)
app.include_router(scheduled_posts.router)
