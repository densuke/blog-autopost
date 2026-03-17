#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""TicketManagerのレースコンディション修正テスト

I/O操作（ファイル削除）がロック保持中に行われると、他スレッドがブロックされる問題を検証。
修正後は、ロック外でファイル削除を行うことでロック保持時間を最小化する。
"""

import os
import tempfile
import threading
import time
from datetime import datetime, timedelta
from unittest.mock import patch, MagicMock

import pytest

from src.web.ticket_manager import TicketManager


class TestTicketManagerIoSeparation:
    """I/O操作とロックの分離テスト"""

    def test_cleanup_media_files_called_outside_lock(self):
        """ファイル削除がロック外で実行されることを確認する

        ロック内でファイル削除が行われると、大量ファイル削除時に
        他スレッドが長時間ブロックされる問題が発生する。
        """
        manager = TicketManager()
        temp_dir = tempfile.mkdtemp()
        media_file = os.path.join(temp_dir, "test.jpg")
        with open(media_file, "w") as f:
            f.write("test")

        ticket_id = manager.create_ticket(["x"], [media_file])

        lock_held_during_io = []

        original_remove = os.remove

        def mock_remove(path):
            # ファイル削除時にロックが保持されているかチェック
            # locked()はRLockで使えないので、try_acquireで確認
            could_acquire = manager._lock.acquire(blocking=False)
            lock_held_during_io.append(not could_acquire)
            if could_acquire:
                manager._lock.release()
            original_remove(path)

        with patch("os.remove", side_effect=mock_remove):
            manager._cleanup_ticket(ticket_id)

        assert media_file not in (f for f in lock_held_during_io)
        # ファイル削除時はロックが保持されていないはず
        assert all(not held for held in lock_held_during_io), (
            "ファイル削除中にロックが保持されている（改善が必要）"
        )

    def test_concurrent_cleanup_and_create(self):
        """クリーンアップと作成が並行実行されても安全に動作する"""
        manager = TicketManager()
        temp_dir = tempfile.mkdtemp()
        errors = []
        created_ids = []

        def create_tickets():
            for _ in range(20):
                tid = manager.create_ticket(["x"])
                created_ids.append(tid)

        # 期限切れチケットを100件作成
        old_tickets = []
        for _ in range(50):
            media_file = os.path.join(temp_dir, f"file_{_}.txt")
            with open(media_file, "w") as f:
                f.write("test")
            tid = manager.create_ticket(["x"], [media_file])
            manager.tickets[tid]["expire_at"] = datetime.now() - timedelta(hours=1)
            old_tickets.append(tid)

        def cleanup():
            try:
                manager.cleanup_expired_tickets()
            except Exception as e:
                errors.append(e)

        # 並行実行
        t1 = threading.Thread(target=create_tickets)
        t2 = threading.Thread(target=cleanup)
        t1.start()
        t2.start()
        t1.join()
        t2.join()

        assert not errors, f"並行実行でエラーが発生: {errors}"
        # 期限切れチケットが削除されていることを確認
        for tid in old_tickets:
            assert tid not in manager.tickets

    def test_lock_not_held_during_file_deletion_in_cleanup_expired(self):
        """cleanup_expired_tickets()でのファイル削除中にロックが取得できる"""
        manager = TicketManager()
        temp_dir = tempfile.mkdtemp()

        # 期限切れチケットとメディアファイルを作成
        media_file = os.path.join(temp_dir, "expired.jpg")
        with open(media_file, "w") as f:
            f.write("test")

        tid = manager.create_ticket(["x"], [media_file])
        manager.tickets[tid]["expire_at"] = datetime.now() - timedelta(hours=1)

        lock_state_during_io = []
        original_remove = os.remove

        def mock_remove(path):
            could_acquire = manager._lock.acquire(blocking=False)
            lock_state_during_io.append(could_acquire)
            if could_acquire:
                manager._lock.release()
            original_remove(path)

        with patch("os.remove", side_effect=mock_remove):
            manager.cleanup_expired_tickets()

        # ファイル削除時はロックが取得可能であるべき
        assert all(lock_state_during_io), (
            "ファイル削除中にロックが取得できなかった（I/O中ロック保持の問題）"
        )

    def test_get_status_cleanup_does_not_hold_lock_during_io(self):
        """get_status()での期限切れクリーンアップ中にロックが取得できる"""
        manager = TicketManager()
        temp_dir = tempfile.mkdtemp()

        media_file = os.path.join(temp_dir, "status_test.jpg")
        with open(media_file, "w") as f:
            f.write("test")

        tid = manager.create_ticket(["x"], [media_file])

        lock_state_during_io = []
        original_remove = os.remove

        def mock_remove(path):
            could_acquire = manager._lock.acquire(blocking=False)
            lock_state_during_io.append(could_acquire)
            if could_acquire:
                manager._lock.release()
            original_remove(path)

        with (
            patch("os.remove", side_effect=mock_remove),
            patch("src.web.ticket_manager.datetime") as mock_dt,
        ):
            mock_dt.now.return_value = datetime.now() + timedelta(hours=25)
            result = manager.get_status(tid, "x")

        assert result is None  # 期限切れなのでNone
        assert not os.path.exists(media_file)  # ファイルが削除されていること
        if lock_state_during_io:
            assert all(lock_state_during_io), (
                "get_status()のクリーンアップ中にI/OがロックをブロックしているI"
            )
