"""SQLite ベースの ScheduledPostStore 実装

既存の ScheduledPostStore インターフェースを SQLite で実装。
既存コードとの互換性を保証しながら SQLite の利点を活用。
"""

from datetime import datetime
from typing import Dict, List, Optional

from sqlalchemy.orm import Session

from src.web.dao import ScheduledPostDAO
from src.web.models import (
    ScheduledPostDB,
    get_session,
    init_db,
)
from src.web.scheduled_post_model import ScheduledPost
from src.web.timezone_utils import ensure_local_timezone


class ScheduledPostStoreSQLite:
    """SQLite ベースの ScheduledPostStore 互換実装"""

    def __init__(self, db_path: str = 'data/scheduled_posts.db'):
        """
        SQLite ベースのデータストアを初期化
        
        Args:
            db_path: SQLite データベースファイルパス
        """
        self.db_path = db_path
        self.engine = init_db(db_path)
        self.dao = None

    def _get_session(self) -> Session:
        """SQLAlchemy セッション取得ヘルパー"""
        return get_session(self.engine)

    # ===== 既存 API との互換性メソッド =====

    def get_all_posts(self, sort_by: Optional[str] = 'date_asc') -> List[ScheduledPost]:
        """
        すべての予約投稿を取得（既存互換性メソッド）
        
        Args:
            sort_by: ソート順序
        
        Returns:
            ScheduledPost オブジェクトのリスト
        """
        session = self._get_session()
        try:
            dao = ScheduledPostDAO(session)
            db_posts = dao.get_all_posts(sort_by=sort_by)

            # SQLAlchemy モデルを ScheduledPost データクラスに変換
            posts = [self._db_to_scheduled_post(db_post) for db_post in db_posts]
            return posts
        finally:
            session.close()

    def get_post_by_id(self, post_id: str) -> Optional[ScheduledPost]:
        """指定されたIDの予約投稿を取得"""
        session = self._get_session()
        try:
            dao = ScheduledPostDAO(session)
            db_post = dao.get_post_by_id(post_id)

            if db_post:
                return self._db_to_scheduled_post(db_post)
            return None
        finally:
            session.close()

    def create_post(self, post: ScheduledPost) -> ScheduledPost:
        """新しい予約投稿を作成"""
        session = self._get_session()
        try:
            db_post = self._scheduled_post_to_db(post, session)
            dao = ScheduledPostDAO(session)
            dao.create_post(db_post)

            return self._db_to_scheduled_post(db_post)
        finally:
            session.close()

    def update_post(self, post_id: str, updates: Dict) -> Optional[ScheduledPost]:
        """既存の予約投稿を更新"""
        session = self._get_session()
        try:
            dao = ScheduledPostDAO(session)
            db_post = dao.update_post(post_id, updates)

            if db_post:
                return self._db_to_scheduled_post(db_post)
            return None
        finally:
            session.close()

    def delete_post(self, post_id: str) -> Optional[str]:
        """指定されたIDの予約投稿を削除"""
        session = self._get_session()
        try:
            dao = ScheduledPostDAO(session)
            success = dao.delete_post(post_id)

            return post_id if success else None
        finally:
            session.close()

    def delete_posts_older_than(
        self,
        cutoff: datetime,
        statuses: Optional[List[str]] = None
    ) -> int:
        """指定した日時以前の投稿を削除"""
        session = self._get_session()
        try:
            dao = ScheduledPostDAO(session)
            deleted_count = dao.delete_posts_older_than(cutoff, statuses)
            return deleted_count
        finally:
            session.close()

    # ===== 新規 SQLite 専用メソッド =====

    def batch_delete_posts(self, post_ids: List[str]) -> int:
        """複数の予約投稿を一括削除（新機能）
        
        Args:
            post_ids: 削除対象の投稿ID リスト
        
        Returns:
            実際に削除された件数
        """
        session = self._get_session()
        try:
            dao = ScheduledPostDAO(session)
            deleted_count = dao.batch_delete_posts(post_ids)
            return deleted_count
        finally:
            session.close()

    def get_paginated_posts(
        self,
        page: int = 1,
        per_page: int = 10,
        sort_by: Optional[str] = 'date_asc',
        status_filter: Optional[List[str]] = None,
        sns_filter: Optional[List[str]] = None,
    ) -> tuple[List[ScheduledPost], int]:
        """ページネーション対応でフィルター付き予約投稿を取得（新機能）
        
        Args:
            page: ページ番号（1から開始）
            per_page: 1ページあたりの件数
            sort_by: ソート順序
            status_filter: ステータスでフィルター
            sns_filter: SNS別フィルター
        
        Returns:
            (ScheduledPost オブジェクトのリスト, 総件数)
        """
        session = self._get_session()
        try:
            dao = ScheduledPostDAO(session)
            db_posts, total_count = dao.get_paginated_posts(
                page=page,
                per_page=per_page,
                sort_by=sort_by,
                status_filter=status_filter,
                sns_filter=sns_filter,
            )

            posts = [self._db_to_scheduled_post(db_post) for db_post in db_posts]
            return posts, total_count
        finally:
            session.close()

    # ===== 内部ヘルパーメソッド =====

    def _db_to_scheduled_post(self, db_post: ScheduledPostDB) -> ScheduledPost:
        """SQLAlchemy モデルを ScheduledPost データクラスに変換"""
        return ScheduledPost(
            id=db_post.id,
            scheduled_at=ensure_local_timezone(db_post.scheduled_at),
            content=db_post.content,
            media_files=db_post.media_files or [],
            target_sns=db_post.target_sns or [],
            status=db_post.status,
            error_message=db_post.error_message,
            created_at=ensure_local_timezone(db_post.created_at),
            updated_at=ensure_local_timezone(db_post.updated_at),
        )

    def _scheduled_post_to_db(
        self,
        post: ScheduledPost,
        session: Session,
    ) -> ScheduledPostDB:
        """ScheduledPost データクラスを SQLAlchemy モデルに変換"""
        return ScheduledPostDB(
            id=post.id,
            scheduled_at=ensure_local_timezone(post.scheduled_at),
            content=post.content,
            media_files=post.media_files or [],
            target_sns=post.target_sns or [],
            status=post.status,
            error_message=post.error_message,
            created_at=ensure_local_timezone(post.created_at),
            updated_at=ensure_local_timezone(post.updated_at),
        )
