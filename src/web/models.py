"""SQLAlchemy ORM モデル定義

SQLiteベースの予約投稿データモデル。
既存JSONスキーマとの互換性を保ちつつ、リレーショナルデータベースの利点を活用。
"""

import uuid
from datetime import datetime

from sqlalchemy import (
    JSON,
    Column,
    DateTime,
    String,
    Text,
    create_engine,
)
from sqlalchemy.ext.declarative import declarative_base
from sqlalchemy.orm import sessionmaker

from src.web.timezone_utils import ensure_local_timezone, now_local

Base = declarative_base()


class ScheduledPostDB(Base):
    """予約投稿のSQLAlchemyモデル"""

    __tablename__ = 'scheduled_posts'

    # プライマリキー
    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))

    # 基本情報
    content = Column(Text, nullable=False)
    scheduled_at = Column(DateTime(timezone=True), nullable=False, index=True)

    # メディア・SNS情報
    media_files = Column(JSON, default=list)  # List[str]
    target_sns = Column(JSON, default=list)  # List[str]

    # ステータス
    status = Column(
        String(50),
        nullable=False,
        default='予約済み',
        index=True  # フィルタリングで頻繁に使用
    )
    error_message = Column(Text, nullable=True)

    # タイムスタンプ
    created_at = Column(DateTime(timezone=True), nullable=False, index=True, default=now_local)
    updated_at = Column(DateTime(timezone=True), nullable=False, default=now_local, onupdate=now_local)

    def __repr__(self):
        return f"<ScheduledPostDB(id={self.id}, status={self.status}, scheduled_at={self.scheduled_at})>"

    def to_dict(self):
        """SQLAlchemy オブジェクトを辞書に変換（JSON互換）"""
        return {
            'id': self.id,
            'scheduled_at': self.scheduled_at.isoformat(),
            'content': self.content,
            'media_files': self.media_files or [],
            'target_sns': self.target_sns or [],
            'status': self.status,
            'error_message': self.error_message,
            'created_at': self.created_at.isoformat(),
            'updated_at': self.updated_at.isoformat(),
        }

    @classmethod
    def from_dict(cls, data: dict):
        """辞書からSQLAlchemyオブジェクトを生成（JSON互換）"""
        scheduled_at = datetime.fromisoformat(data['scheduled_at']) if data.get('scheduled_at') else None
        created_at = datetime.fromisoformat(data['created_at']) if data.get('created_at') else now_local()
        updated_at = datetime.fromisoformat(data['updated_at']) if data.get('updated_at') else now_local()

        scheduled_at = ensure_local_timezone(scheduled_at)
        created_at = ensure_local_timezone(created_at)
        updated_at = ensure_local_timezone(updated_at)

        return cls(
            id=data.get('id', str(uuid.uuid4())),
            scheduled_at=scheduled_at,
            content=data['content'],
            media_files=data.get('media_files', []),
            target_sns=data.get('target_sns', []),
            status=data.get('status', '予約済み'),
            error_message=data.get('error_message'),
            created_at=created_at,
            updated_at=updated_at,
        )


# データベース初期化ユーティリティ
def get_db_engine(db_path: str = 'data/scheduled_posts.db'):
    """SQLiteエンジンを取得・初期化"""
    engine = create_engine(
        f'sqlite:///{db_path}',
        connect_args={'check_same_thread': False},  # SQLiteの同一スレッド制限を緩和
        echo=False  # SQL ログを出力しない（デバッグ時は True に変更）
    )
    return engine


def init_db(db_path: str = 'data/scheduled_posts.db'):
    """データベーステーブルを初期化"""
    engine = get_db_engine(db_path)
    Base.metadata.create_all(engine)
    return engine


def get_session(engine):
    """SQLAlchemy セッションを取得"""
    Session = sessionmaker(bind=engine)
    return Session()
