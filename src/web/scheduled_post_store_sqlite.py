"""SQLite ベースの ScheduledPostStore 実装

既存の ScheduledPostStore インターフェースを SQLite で実装。
既存コードとの互換性を保証しながら SQLite の利点を活用。
"""

from datetime import datetime, timedelta
from typing import Dict, List, Optional, cast

from sqlalchemy.orm import Session

from src.web.dao import ScheduledPostDAO
from src.web.models import (
    ScheduledPostDB,
    get_session,
    init_db,
)
from src.web.scheduled_post_model import ScheduledPost
from src.web.timezone_utils import ensure_local_timezone, now_local


class ScheduledPostStoreSQLite:
    """SQLite ベースの ScheduledPostStore 互換実装"""

    def __init__(self, db_path: str = "data/scheduled_posts.db"):
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

    def get_all_posts(self, sort_by: Optional[str] = "date_asc") -> List[ScheduledPost]:
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
        self, cutoff: datetime, statuses: Optional[List[str]] = None
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
        sort_by: Optional[str] = "date_asc",
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

    def get_posts_by_sns_and_time(
        self, sns_name: str, scheduled_at: datetime, tolerance_minutes: int = 0
    ) -> List[ScheduledPost]:
        """指定されたSNS・時刻の予約投稿を取得する。

        Args:
            sns_name: SNS名
            scheduled_at: 予約時刻
            tolerance_minutes: 時刻の許容範囲(分)

        Returns:
            該当する予約投稿のリスト
        """
        session = self._get_session()
        try:
            dao = ScheduledPostDAO(session)

            normalized_scheduled = ensure_local_timezone(scheduled_at)
            if normalized_scheduled is None:
                normalized_scheduled = scheduled_at
            if normalized_scheduled.tzinfo is None:
                normalized_scheduled = normalized_scheduled.replace(
                    tzinfo=now_local().tzinfo
                )

            # 許容範囲を考慮した時刻範囲を計算
            start_time = normalized_scheduled - timedelta(minutes=tolerance_minutes)
            end_time = normalized_scheduled + timedelta(minutes=tolerance_minutes)

            # DAOを使用してフィルタリング済みの投稿を取得
            db_posts = dao.get_posts_by_sns_and_time(sns_name, start_time, end_time)

            return [self._db_to_scheduled_post(db_post) for db_post in db_posts]
        finally:
            session.close()

    # ===== 内部ヘルパーメソッド =====

    def _db_to_scheduled_post(self, db_post: ScheduledPostDB) -> ScheduledPost:
        """SQLAlchemy モデルを ScheduledPost データクラスに変換"""
        # Column 型を明示的に str/datetime にキャスト
        db_scheduled_at = cast(datetime, db_post.scheduled_at)
        db_created_at = cast(datetime, db_post.created_at)
        db_updated_at = cast(Optional[datetime], db_post.updated_at) or db_created_at
        db_media_files = cast(List[str], db_post.media_files)
        db_target_sns = cast(List[str], db_post.target_sns)

        scheduled_at_tz = ensure_local_timezone(db_scheduled_at)
        created_at_tz = ensure_local_timezone(db_created_at)
        updated_at_tz = ensure_local_timezone(db_updated_at)

        return ScheduledPost(
            id=str(db_post.id),
            scheduled_at=scheduled_at_tz or db_scheduled_at,
            content=str(db_post.content),
            media_files=db_media_files or [],
            target_sns=db_target_sns or [],
            status=str(db_post.status),
            error_message=str(db_post.error_message) if db_post.error_message else None,
            created_at=created_at_tz or db_created_at,
            updated_at=updated_at_tz or db_updated_at,
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
