import http.cookies
import secrets
from typing import Optional

from itsdangerous import URLSafeSerializer
from starlette.datastructures import MutableHeaders
from starlette.requests import Request
from starlette.types import ASGIApp, Message, Receive, Scope, Send
from starlette_csrf.middleware import CSRFMiddleware as _BaseCSRFMiddleware


class CSRFCookieMiddleware:
    """
    Ensure a CSRF cookie is always available so templates can embed the token.

    This middleware generates a signed CSRF token when the incoming request
    lacks the cookie. The generated token is exposed via ``request.state``
    so downstream handlers can easily inject it into rendered templates.
    """

    def __init__(
        self,
        app: ASGIApp,
        secret: str,
        *,
        cookie_name: str = "csrftoken",
        cookie_path: str = "/",
        cookie_domain: Optional[str] = None,
        cookie_secure: bool = False,
        cookie_httponly: bool = False,
        cookie_samesite: str = "lax",
    ) -> None:
        self.app = app
        self.serializer = URLSafeSerializer(secret, "csrftoken")
        self.cookie_name = cookie_name
        self.cookie_path = cookie_path
        self.cookie_domain = cookie_domain
        self.cookie_secure = cookie_secure
        self.cookie_httponly = cookie_httponly
        self.cookie_samesite = cookie_samesite

    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        if scope["type"] != "http":  # pragma: no cover
            await self.app(scope, receive, send)
            return

        headers = MutableHeaders(scope=scope)
        cookie_header = headers.get("cookie")

        cookies = http.cookies.SimpleCookie()
        if cookie_header:
            cookies.load(cookie_header)

        morsel = cookies.get(self.cookie_name)
        token = morsel.value if morsel else None
        generated_new_token = False

        if token is None:
            token = self._generate_csrf_token()
            cookies[self.cookie_name] = token
            headers["cookie"] = cookies.output(header="", sep="; ").strip()
            generated_new_token = True

        state = scope.setdefault("state", {})
        state["csrf_token"] = token

        async def send_wrapper(message: Message) -> None:
            if generated_new_token and message["type"] == "http.response.start":
                response_headers = MutableHeaders(scope=message)
                cookie = http.cookies.SimpleCookie()
                cookie[self.cookie_name] = token
                cookie[self.cookie_name]["path"] = self.cookie_path
                cookie[self.cookie_name]["secure"] = self.cookie_secure
                cookie[self.cookie_name]["httponly"] = self.cookie_httponly
                cookie[self.cookie_name]["samesite"] = self.cookie_samesite
                if self.cookie_domain is not None:
                    cookie[self.cookie_name]["domain"] = self.cookie_domain
                response_headers.append("set-cookie", cookie.output(header="").strip())

            await send(message)

        await self.app(scope, receive, send_wrapper)

    def _generate_csrf_token(self) -> str:
        return str(self.serializer.dumps(secrets.token_urlsafe(128)))


class FormCSRFMiddleware(_BaseCSRFMiddleware):
    """
    Extend ``starlette-csrf`` middleware to also accept tokens submitted
    via form fields or JSON bodies in addition to the ``X-CSRFToken`` header.
    """

    async def _get_submitted_csrf_token(self, request: Request) -> Optional[str]:
        header_token = request.headers.get(self.header_name)
        if header_token:
            return header_token

        content_type = request.headers.get("content-type", "")

        if "application/json" in content_type:
            try:
                payload = await request.json()
            except ValueError:
                payload = None
            if isinstance(payload, dict):
                body_token = payload.get("csrf_token")
                if body_token:
                    return body_token

        if "application/x-www-form-urlencoded" in content_type or "multipart/form-data" in content_type:
            form = await request.form()
            form_token = form.get("csrf_token")
            if form_token:
                return form_token

        query_token = request.query_params.get("csrf_token")
        if query_token:
            return query_token

        return None
