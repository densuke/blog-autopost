from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import List, Optional
import uuid

@dataclass
class ScheduledPost:
    scheduled_at: datetime
    content: str
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    media_files: List[str] = field(default_factory=list)
    target_sns: List[str] = field(default_factory=list)
    status: str = "予約済み"  # 予約済み, 実行済み, 失敗
    error_message: Optional[str] = None
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    updated_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    
    def __post_init__(self):
        """
        タイムゾーンの正規化処理
        naiveなdatetimeをUTCのawareなdatetimeに変換します。
        """
        if self.scheduled_at and self.scheduled_at.tzinfo is None:
            self.scheduled_at = self.scheduled_at.replace(tzinfo=timezone.utc)
        
        if self.created_at and self.created_at.tzinfo is None:
            self.created_at = self.created_at.replace(tzinfo=timezone.utc)

        if self.updated_at and self.updated_at.tzinfo is None:
            self.updated_at = self.updated_at.replace(tzinfo=timezone.utc)

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
        # fromisoformatはawareなdatetimeを返すことがある
        scheduled_at = datetime.fromisoformat(data["scheduled_at"]) if data.get("scheduled_at") else None
        created_at = datetime.fromisoformat(data["created_at"]) if data.get("created_at") else datetime.now(timezone.utc)
        updated_at = datetime.fromisoformat(data["updated_at"]) if data.get("updated_at") else datetime.now(timezone.utc)

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
