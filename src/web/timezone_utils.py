"""Utilities for normalizing datetimes to the local timezone."""
from __future__ import annotations

from datetime import datetime, timezone
from typing import Optional

_LOCALIZED_NOW = datetime.now().astimezone()
LOCAL_TIMEZONE = _LOCALIZED_NOW.tzinfo or timezone.utc


def get_local_timezone():
    """Return the system's local timezone info."""
    return LOCAL_TIMEZONE


def now_local() -> datetime:
    """Return the current time localized to the system timezone."""
    return datetime.now(LOCAL_TIMEZONE)


def ensure_local_timezone(dt: Optional[datetime]) -> Optional[datetime]:
    """Ensure ``dt`` is timezone-aware and expressed in the local timezone."""
    if dt is None:
        return None
    if dt.tzinfo is None:
        return dt.replace(tzinfo=LOCAL_TIMEZONE)
    return dt.astimezone(LOCAL_TIMEZONE)
