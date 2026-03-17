"""セキュリティレスポンスヘッダーミドルウェア

全HTTPレスポンスにセキュリティ関連ヘッダーを付与する。
ブラウザのセキュリティ機能を有効化し、一般的な攻撃を緩和する。
"""
from starlette.types import ASGIApp, Message, Receive, Scope, Send
from starlette.datastructures import MutableHeaders

# 付与するセキュリティヘッダーの定義
SECURITY_HEADERS: dict[str, str] = {
    # MIMEタイプスニッフィング防止
    "X-Content-Type-Options": "nosniff",
    # クリックジャッキング防止
    "X-Frame-Options": "DENY",
    # 旧ブラウザ向けXSSフィルタ有効化
    "X-XSS-Protection": "1; mode=block",
    # リファラー情報の漏洩を制限
    "Referrer-Policy": "strict-origin-when-cross-origin",
    # 不要なブラウザAPI権限を無効化
    "Permissions-Policy": "camera=(), microphone=(), geolocation=()",
}


class SecurityHeadersMiddleware:
    """全レスポンスにセキュリティヘッダーを付与するASGIミドルウェア。

    Args:
        app: 内部ASGIアプリケーション。
        headers: 付与するヘッダーの辞書。省略時は SECURITY_HEADERS を使用。
    """

    def __init__(self, app: ASGIApp, headers: dict[str, str] | None = None) -> None:
        self.app = app
        self.headers = headers if headers is not None else SECURITY_HEADERS

    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return

        async def send_with_security_headers(message: Message) -> None:
            if message["type"] == "http.response.start":
                response_headers = MutableHeaders(scope=message)
                for name, value in self.headers.items():
                    # 既にヘッダーが設定されている場合は上書きしない
                    if name.lower() not in {k.lower() for k in response_headers.keys()}:
                        response_headers.append(name, value)
            await send(message)

        await self.app(scope, receive, send_with_security_headers)
