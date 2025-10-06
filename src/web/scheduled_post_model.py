from dataclasses import dataclass, field
from datetime import datetime
from typing import List, Optional
import uuid

from src.web.timezone_utils import ensure_local_timezone, now_local


@dataclass
class ScheduledPost:
    scheduled_at: datetime
    content: str
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    media_files: List[str] = field(default_factory=list)
    target_sns: List[str] = field(default_factory=list)
    status: str = "予約済み"  # 予約済み, 実行済み, 失敗
    error_message: Optional[str] = None
    created_at: datetime = field(default_factory=now_local)
    updated_at: datetime = field(default_factory=now_local)

    def __post_init__(self):
        """
        タイムゾーンの正規化処理
        naiveなdatetimeをローカルタイムのawareなdatetimeに変換します。
        """
        if self.scheduled_at:
            self.scheduled_at = ensure_local_timezone(self.scheduled_at)

        if self.created_at:
            self.created_at = ensure_local_timezone(self.created_at)

        if self.updated_at:
            self.updated_at = ensure_local_timezone(self.updated_at)

    def to_dict(self):
        return {
            "id": self.id,
            "scheduled_at": self.scheduled_at.isoformat(),
            "content": self.content,
            "media_files": self.media_files,
            "target_sns": self.target_sns,
            "status": self.status,
            "error_message": self.error_message,
            "created_at": self.created_at.isoformat(),
            "updated_at": self.updated_at.isoformat(),
        }

    @classmethod
    def from_dict(cls, data: dict):
        scheduled_at = datetime.fromisoformat(data["scheduled_at"]) if data.get("scheduled_at") else None
        created_at = datetime.fromisoformat(data["created_at"]) if data.get("created_at") else now_local()
        updated_at = datetime.fromisoformat(data["updated_at"]) if data.get("updated_at") else now_local()

        scheduled_at = ensure_local_timezone(scheduled_at)
        created_at = ensure_local_timezone(created_at)
        updated_at = ensure_local_timezone(updated_at)

        return cls(
            id=data["id"],
            scheduled_at=scheduled_at,
            content=data["content"],
            media_files=data.get("media_files", []),
            target_sns=data.get("target_sns", []),
            status=data.get("status", "予約済み"),
            error_message=data.get("error_message"),
            created_at=created_at,
            updated_at=updated_at,
        )
