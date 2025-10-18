"""DAO層（Data Access Object）

SQLAlchemy を使用した CRUD 操作の抽象化層。
既存の ScheduledPostStore の機能を SQLite で実装。
"""

from datetime import datetime
from typing import Dict, List, Optional

from sqlalchemy import or_
from sqlalchemy.orm import Session

from src.web.models import ScheduledPostDB
from src.web.timezone_utils import ensure_local_timezone


class ScheduledPostDAO:
    """予約投稿のデータアクセスオブジェクト"""

    def __init__(self, session: Session):
        self.session = session

    # ===== READ 操作 =====

    def get_all_posts(self, sort_by: Optional[str] = 'date_asc') -> List[ScheduledPostDB]:
        """
        すべての予約投稿を取得し、指定されたキーでソート。
        
        Args:
            sort_by: ソート順序
                - 'date_asc': 投稿日時（昇順）
                - 'date_desc': 投稿日時（降順）
                - 'status_failed': ステータス（失敗優先）
                - 'status_completed': ステータス（完了優先）
        
        Returns:
            ScheduledPostDB オブジェクトのリスト
        """
        query = self.session.query(ScheduledPostDB)

        if sort_by == 'date_desc':
            query = query.order_by(ScheduledPostDB.scheduled_at.desc())
        elif sort_by == 'status_failed':
            # ステータス順: 失敗 -> 予約済み -> 実行済み
            status_priority = {
                '失敗': 0,
                '予約済み': 1,
                '実行済み': 2,
            }
            # SQLite での CASE 文を用いたソート
            from sqlalchemy import case
            case_stmt = case(
                [(ScheduledPostDB.status == k, v) for k, v in status_priority.items()],
                else_=99
            )
            query = query.order_by(case_stmt, ScheduledPostDB.scheduled_at)
        elif sort_by == 'status_completed':
            # ステータス順: 実行済み -> 予約済み -> 失敗
            status_priority = {
                '実行済み': 0,
                '予約済み': 1,
                '失敗': 2,
            }
            from sqlalchemy import case
            case_stmt = case(
                [(ScheduledPostDB.status == k, v) for k, v in status_priority.items()],
                else_=99
            )
            query = query.order_by(case_stmt, ScheduledPostDB.scheduled_at)
        else:  # date_asc (default)
            query = query.order_by(ScheduledPostDB.scheduled_at.asc())

        return query.all()

    def get_paginated_posts(
        self,
        page: int = 1,
        per_page: int = 10,
        sort_by: Optional[str] = 'date_asc',
        status_filter: Optional[List[str]] = None,
        sns_filter: Optional[List[str]] = None,
    ) -> tuple[List[ScheduledPostDB], int]:
        """
        ページネーション対応でフィルター付き予約投稿を取得。
        
        Args:
            page: ページ番号（1から開始）
            per_page: 1ページあたりの件数
            sort_by: ソート順序
            status_filter: ステータスでフィルター（例：['失敗', '予約済み']）
            sns_filter: SNS別フィルター（例：['x', 'bluesky']）
        
        Returns:
            (ScheduledPostDB オブジェクトのリスト, 総件数)
        """
        query = self.session.query(ScheduledPostDB)

        # ステータスフィルター
        if status_filter:
            query = query.filter(ScheduledPostDB.status.in_(status_filter))

        # SNS フィルター（JSON配列内に含まれるかチェック）
        if sns_filter:
            # JSON フィルター: target_sns 配列に指定の SNS が含まれる
            or_conditions = [
                ScheduledPostDB.target_sns.contains(sns) for sns in sns_filter
            ]
            query = query.filter(or_(*or_conditions))

        # 総件数を取得（フィルター後）
        total_count = query.count()

        # ソート
        if sort_by == 'date_desc':
            query = query.order_by(ScheduledPostDB.scheduled_at.desc())
        elif sort_by == 'status_failed':
            from sqlalchemy import case
            status_priority = {'失敗': 0, '予約済み': 1, '実行済み': 2}
            case_stmt = case(
                [(ScheduledPostDB.status == k, v) for k, v in status_priority.items()],
                else_=99
            )
            query = query.order_by(case_stmt, ScheduledPostDB.scheduled_at)
        elif sort_by == 'status_completed':
            from sqlalchemy import case
            status_priority = {'実行済み': 0, '予約済み': 1, '失敗': 2}
            case_stmt = case(
                [(ScheduledPostDB.status == k, v) for k, v in status_priority.items()],
                else_=99
            )
            query = query.order_by(case_stmt, ScheduledPostDB.scheduled_at)
        else:  # date_asc
            query = query.order_by(ScheduledPostDB.scheduled_at.asc())

        # ページネーション
        offset = (page - 1) * per_page
        posts = query.offset(offset).limit(per_page).all()

        return posts, total_count

    def get_post_by_id(self, post_id: str) -> Optional[ScheduledPostDB]:
        """指定されたIDの予約投稿を取得"""
        return self.session.query(ScheduledPostDB).filter(
            ScheduledPostDB.id == post_id
        ).first()

    # ===== CREATE 操作 =====

    def create_post(self, post: ScheduledPostDB) -> ScheduledPostDB:
        """新しい予約投稿を作成"""
        self.session.add(post)
        self.session.commit()
        return post

    # ===== UPDATE 操作 =====

    def update_post(self, post_id: str, updates: Dict) -> Optional[ScheduledPostDB]:
        """既存の予約投稿を更新"""
        post = self.get_post_by_id(post_id)
        if not post:
            return None

        for key, value in updates.items():
            if isinstance(value, datetime):
                value = ensure_local_timezone(value)
            setattr(post, key, value)

        updated_at_tz = ensure_local_timezone(datetime.now())
        post.updated_at = updated_at_tz if updated_at_tz is not None else datetime.now()
        self.session.commit()
        return post

    # ===== DELETE 操作 =====

    def delete_post(self, post_id: str) -> bool:
        """指定されたIDの予約投稿を削除"""
        post = self.get_post_by_id(post_id)
        if not post:
            return False

        self.session.delete(post)
        self.session.commit()
        return True

    def batch_delete_posts(self, post_ids: List[str]) -> int:
        """複数の予約投稿を一括削除
        
        Args:
            post_ids: 削除対象の投稿ID リスト
        
        Returns:
            実際に削除された件数
        """
        if not post_ids:
            return 0

        deleted_count = self.session.query(ScheduledPostDB).filter(
            ScheduledPostDB.id.in_(post_ids)
        ).delete(synchronize_session=False)

        self.session.commit()
        return deleted_count

    def delete_posts_older_than(
        self,
        cutoff: datetime,
        statuses: Optional[List[str]] = None,
    ) -> int:
        """指定した日時以前の投稿を削除
        
        Args:
            cutoff: カットオフ日時
            statuses: 削除対象のステータス（例：['実行済み', '失敗']）
        
        Returns:
            削除された件数
        """
        cutoff_tz = ensure_local_timezone(cutoff)
        if cutoff_tz is None:
            cutoff_tz = cutoff

        query = self.session.query(ScheduledPostDB).filter(
            ScheduledPostDB.updated_at <= cutoff_tz
        )

        if statuses:
            query = query.filter(ScheduledPostDB.status.in_(statuses))

        deleted_count = query.delete(synchronize_session=False)
        self.session.commit()

        return deleted_count
