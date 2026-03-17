"""レート制限の共有インスタンス

循環インポートを避けるため、limiter を独立モジュールで定義し
main_web.py とルートモジュールの両方からインポートする。
"""
from slowapi import Limiter
from slowapi.util import get_remote_address

# IPアドレスベースのレート制限インスタンス（アプリ全体で共有）
limiter = Limiter(key_func=get_remote_address)
