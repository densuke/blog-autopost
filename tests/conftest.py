"""テスト共通フィクスチャ"""
import pytest


@pytest.fixture(autouse=True)
def reset_rate_limiter():
    """各テスト前後にレート制限の状態をリセットする（テスト間の状態漏れ防止）"""
    from src.web.rate_limiter import limiter
    limiter.reset()
    yield
    limiter.reset()
