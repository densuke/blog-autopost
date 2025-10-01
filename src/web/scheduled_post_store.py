import json
from pathlib import Path
from typing import List, Dict, Optional
from datetime import datetime
import uuid

from src.web.scheduled_post_model import ScheduledPost

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

    # CRUD operations
    def get_all_posts(self) -> List[ScheduledPost]:
        """
        すべての予約投稿を取得します。
        """
        return self._read_posts()

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
                    setattr(post, key, value)
                post.updated_at = datetime.now() # 更新日時を更新
                posts[i] = post
                self._write_posts(posts)
                return post
        return None

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