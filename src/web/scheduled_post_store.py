import json
from pathlib import Path
from typing import List, Dict, Optional
from datetime import datetime
import uuid
import os

from src.web.scheduled_post_model import ScheduledPost
from src.web.timezone_utils import ensure_local_timezone, now_local

class ScheduledPostStore:
    def __init__(self, file_path: Path):
        self.file_path = file_path
        self._initialize_file()

    def _initialize_file(self):
        """
        JSONファイルが存在しない場合に初期化します。
        """
        if not self.file_path.exists():
            self.file_path.parent.mkdir(parents=True, exist_ok=True)
            with open(self.file_path, 'w', encoding='utf-8') as f:
                json.dump([], f, ensure_ascii=False, indent=4)
            # セキュリティ: ファイルパーミッションを設定（ファイル所有者のみ読み書き可能）
            os.chmod(self.file_path, 0o600)

    def _read_posts(self) -> List[ScheduledPost]:
        """
        JSONファイルからすべての予約投稿を読み込みます。
        """
        with open(self.file_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
            return [ScheduledPost.from_dict(item) for item in data]

    def _write_posts(self, posts: List[ScheduledPost]):
        """
        すべての予約投稿をJSONファイルに書き込みます。
        """
        with open(self.file_path, 'w', encoding='utf-8') as f:
            json.dump([post.to_dict() for post in posts], f, ensure_ascii=False, indent=4)
        # セキュリティ: ファイルパーミッションを維持
        os.chmod(self.file_path, 0o600)

    # CRUD operations
    def get_all_posts(self, sort_by: Optional[str] = 'date_asc') -> List[ScheduledPost]:
        """
        すべての予約投稿を取得し、指定されたキーでソートします。
        """
        posts = self._read_posts()

        if sort_by == 'date_desc':
            posts.sort(key=lambda p: p.scheduled_at, reverse=True)
        elif sort_by == 'status_failed':
            # 失敗 -> 予約済み -> 実行済みの順
            status_order = {"失敗": 0, "予約済み": 1, "実行済み": 2}
            posts.sort(key=lambda p: (status_order.get(p.status, 99), p.scheduled_at))
        elif sort_by == 'status_completed':
            # 実行済み -> 予約済み -> 失敗の順
            status_order = {"実行済み": 0, "予約済み": 1, "失敗": 2}
            posts.sort(key=lambda p: (status_order.get(p.status, 99), p.scheduled_at))
        else: # date_asc (default)
            posts.sort(key=lambda p: p.scheduled_at)
            
        return posts

    def get_post_by_id(self, post_id: str) -> Optional[ScheduledPost]:
        """
        指定されたIDの予約投稿を取得します。
        """
        posts = self._read_posts()
        return next((post for post in posts if post.id == post_id), None)

    def create_post(self, post: ScheduledPost) -> ScheduledPost:
        """
        新しい予約投稿を作成します。
        """
        posts = self._read_posts()
        posts.append(post)
        self._write_posts(posts)
        return post

    def update_post(self, post_id: str, updates: Dict) -> Optional[ScheduledPost]:
        """
        既存の予約投稿を更新します。
        """
        posts = self._read_posts()
        for i, post in enumerate(posts):
            if post.id == post_id:
                for key, value in updates.items():
                    if isinstance(value, datetime):
                        value = ensure_local_timezone(value)
                    setattr(post, key, value)
                post.updated_at = now_local()  # 更新日時を更新
                posts[i] = post
                self._write_posts(posts)
                return post
        return None

    def delete_posts_older_than(self, cutoff: datetime, statuses: Optional[List[str]] = None) -> int:
        """
        指定した日時以前の投稿を削除します。
        """
        posts = self._read_posts()
        kept_posts = []
        removed = 0

        for post in posts:
            status_match = statuses is None or post.status in statuses
            if not status_match:
                kept_posts.append(post)
                continue

            reference_time = post.updated_at or post.scheduled_at
            reference_time = ensure_local_timezone(reference_time) if reference_time else None

            if reference_time and reference_time <= cutoff:
                removed += 1
            else:
                kept_posts.append(post)

        if removed > 0:
            self._write_posts(kept_posts)

        return removed


    def delete_post(self, post_id: str) -> Optional[str]:
        """
        指定されたIDの予約投稿を削除します。
        """
        posts = self._read_posts()
        initial_len = len(posts)
        posts = [post for post in posts if post.id != post_id]
        if len(posts) < initial_len:
            self._write_posts(posts)
            return post_id
        return None