"""ファイルアップロードのバリデーション

サイズ制限とMIME型許可リストを適用し、不正なファイルアップロードを防止する。
"""
from __future__ import annotations

from typing import Optional

from fastapi import UploadFile

# 最大アップロードサイズ: 10MB
MAX_UPLOAD_SIZE_BYTES = 10 * 1024 * 1024

# 許可するMIME型（画像・動画のみ）
ALLOWED_MIME_TYPES: frozenset[str] = frozenset(
    [
        "image/jpeg",
        "image/png",
        "image/gif",
        "image/webp",
        "video/mp4",
        "video/quicktime",
    ]
)


def validate_upload_file(file: UploadFile) -> Optional[str]:
    """アップロードファイルのバリデーションを行う。

    ファイル名が空の場合はスキップ（エラーなし）とする。
    MIME型が許可リストにない場合、またはサイズが上限を超える場合はエラーを返す。

    Args:
        file: バリデーション対象のアップロードファイル。

    Returns:
        エラーメッセージ文字列。問題なければ None。
    """
    if not file.filename:
        return None

    # MIME型チェック
    content_type = file.content_type or ""
    # content_type に "image/jpeg; charset=..." のようにパラメータが含まれる場合を考慮
    mime_base = content_type.split(";")[0].strip()
    if mime_base not in ALLOWED_MIME_TYPES:
        allowed = ", ".join(sorted(ALLOWED_MIME_TYPES))
        return (
            f"許可されていないファイル形式です: {mime_base}。"
            f"許可形式: {allowed}"
        )

    # サイズチェック（size が取得できる場合のみ）
    size = getattr(file, "size", None)
    if size is not None and size > MAX_UPLOAD_SIZE_BYTES:
        limit_mb = MAX_UPLOAD_SIZE_BYTES // (1024 * 1024)
        return f"ファイルサイズが上限（{limit_mb}MB）を超えています: {size} bytes"

    return None


def validate_upload_files(files: list[UploadFile]) -> Optional[str]:
    """複数ファイルのバリデーションをまとめて行う。

    Args:
        files: バリデーション対象のアップロードファイルリスト。

    Returns:
        最初に見つかったエラーメッセージ。全て正常なら None。
    """
    for file in files:
        error = validate_upload_file(file)
        if error:
            return error
    return None
