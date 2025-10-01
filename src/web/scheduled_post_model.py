from dataclasses import dataclass, field
from datetime import datetime
from typing import List, Optional
import uuid

@dataclass
class ScheduledPost:
    scheduled_at: datetime = field(metadata={'by_alias': True})
    content: str
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    media_files: List[str] = field(default_factory=list)
    target_sns: List[str] = field(default_factory=list)
    status: str = "予約済み"  # 予約済み, 実行済み, 失敗
    error_message: Optional[str] = None
    created_at: datetime = field(default_factory=datetime.now)
    updated_at: datetime = field(default_factory=datetime.now)
    _initialized: bool = field(default=False, init=False, repr=False)
    
    def __post_init__(self):
        """バリデーション処理"""
        # 予約済み投稿の場合、scheduled_atは未来でなければならない
        # ただし、過去の予約投稿をスケジューラーが処理するシナリオを考慮し、
        # ここでは厳密な未来日時チェックは行わない。
        # APIレベルで未来日時を強制する。
        pass
        # 初期化完了フラグを設定（このsetはバリデーションをバイパスする）
        object.__setattr__(self, '_initialized', True)
    
    def __setattr__(self, name, value):
        """属性の設定時のバリデーション"""
        # 初期化中または内部フラグの場合はバリデーションをスキップ
        if name == '_initialized' or not getattr(self, '_initialized', False):
            object.__setattr__(self, name, value)
            return
        
        # 実行済みまたは失敗の投稿は編集不可（特定フィールド以外）
        if self.status in ["実行済み", "失敗"]:
            # これらのフィールドは更新可能
            if name not in ['status', 'error_message', 'updated_at', 'scheduled_at']:
                raise ValueError(f"Cannot edit a post with status '{self.status}'")
        
        object.__setattr__(self, name, value)

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
        return cls(
            id=data["id"],
            scheduled_at=datetime.fromisoformat(data["scheduled_at"]),
            content=data["content"],
            media_files=data.get("media_files", []),
            target_sns=data.get("target_sns", []),
            status=data.get("status", "予約済み"),
            error_message=data.get("error_message"),
            created_at=datetime.fromisoformat(data["created_at"]) if "created_at" in data else datetime.now(),
            updated_at=datetime.fromisoformat(data["updated_at"]) if "updated_at" in data else datetime.now(),
        )