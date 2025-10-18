#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""チケットベースの投稿状態管理システム

複数SNSへの投稿状態を個別に追跡するためのチケット管理システム。
各チケットは24時間の有効期限を持ち、自動クリーンアップされる。
"""

import threading
import uuid
from datetime import datetime, timedelta
from typing import Dict, List, Optional


class TicketManager:
    """投稿チケット管理クラス

    チケットの発行、状態更新、クリーンアップを管理する。
    スレッドセーフな実装。
    """

    def __init__(self, ticket_lifetime_hours: int = 24):
        """
        Args:
            ticket_lifetime_hours: チケットの有効期限（時間）
        """
        self.tickets: Dict[str, dict] = {}
        self.ticket_lifetime_hours = ticket_lifetime_hours
        self._lock = threading.Lock()

    def create_ticket(self, sns_list: List[str], media_files: Optional[List[str]] = None) -> str:
        """新規チケットを発行

        Args:
            sns_list: 投稿対象のSNS名リスト
            media_files: メディアファイルパスのリスト（クリーンアップ用）

        Returns:
            発行されたチケットID
        """
        ticket_id = str(uuid.uuid4())
        now = datetime.now()

        # 各SNSの初期状態を設定
        sns_statuses = {}
        for sns in sns_list:
            sns_statuses[sns] = {
                'status': 'processing',
                'message': None,
                'updated_at': now
            }

        with self._lock:
            self.tickets[ticket_id] = {
                'created_at': now,
                'expire_at': now + timedelta(hours=self.ticket_lifetime_hours),
                'sns_statuses': sns_statuses,
                'media_files': media_files or [],
            }

        return ticket_id

    def get_status(self, ticket_id: str, sns: str) -> Optional[dict]:
        """特定SNSの投稿状態を取得

        Args:
            ticket_id: チケットID
            sns: SNS名

        Returns:
            状態辞書 {'status': str, 'message': str, 'updated_at': datetime}
            チケットが存在しないか期限切れの場合はNone
        """
        with self._lock:
            if ticket_id not in self.tickets:
                return None

            ticket = self.tickets[ticket_id]

            # 期限チェック
            if ticket['expire_at'] < datetime.now():
                self._cleanup_ticket(ticket_id)
                return None

            return ticket['sns_statuses'].get(sns)

    def update_status(self, ticket_id: str, sns: str, status: str, message: Optional[str] = None) -> bool:
        """SNSの投稿状態を更新

        Args:
            ticket_id: チケットID
            sns: SNS名
            status: 新しい状態 ('processing', 'success', 'failed', 'error')
            message: ステータスメッセージ

        Returns:
            更新成功ならTrue、チケットが存在しない場合はFalse
        """
        # 状態遷移の検証
        valid_statuses = ['processing', 'success', 'failed', 'error']
        if status not in valid_statuses:
            raise ValueError(f"Invalid status: {status}. Must be one of {valid_statuses}")

        with self._lock:
            if ticket_id not in self.tickets:
                return False

            ticket = self.tickets[ticket_id]

            # 期限チェック
            if ticket['expire_at'] < datetime.now():
                self._cleanup_ticket(ticket_id)
                return False

            if sns not in ticket['sns_statuses']:
                return False

            # 状態更新
            ticket['sns_statuses'][sns] = {
                'status': status,
                'message': message,
                'updated_at': datetime.now()
            }

            # 全SNSが完了したかチェック
            all_completed = all(
                s['status'] in ['success', 'failed', 'error']
                for s in ticket['sns_statuses'].values()
            )

            # 全完了なら自動クリーンアップ（24時間待たずに）
            if all_completed:
                # ただし即座には削除せず、少し保持する（結果取得のため）
                # 実際のクリーンアップは get_status で期限切れ時に実行
                pass

            return True

    def _cleanup_ticket(self, ticket_id: str):
        """チケットのクリーンアップ（内部用）

        メディアファイルを削除し、チケット情報を削除する。

        Args:
            ticket_id: チケットID
        """
        if ticket_id not in self.tickets:
            return

        ticket = self.tickets[ticket_id]

        # メディアファイルの削除
        import os
        for media_file in ticket.get('media_files', []):
            try:
                if os.path.exists(media_file):
                    os.remove(media_file)
            except Exception:
                pass  # 削除失敗は無視

        # チケット削除
        del self.tickets[ticket_id]

    def cleanup_expired_tickets(self):
        """期限切れチケットの一括クリーンアップ

        定期的に呼び出すことで、期限切れチケットをクリーンアップする。
        """
        now = datetime.now()
        expired_tickets = []

        with self._lock:
            for ticket_id, ticket in self.tickets.items():
                if ticket['expire_at'] < now:
                    expired_tickets.append(ticket_id)

            for ticket_id in expired_tickets:
                self._cleanup_ticket(ticket_id)

        return len(expired_tickets)
