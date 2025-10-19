#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""ticket_manager.py のテスト

チケット管理システムの各機能をテストする。
"""

import os
import tempfile
from datetime import datetime, timedelta
from unittest.mock import patch

import pytest

from src.web.ticket_manager import TicketManager


class TestTicketManager:
    """TicketManager クラスのテスト"""

    def setup_method(self):
        """各テスト前に実行"""
        self.manager = TicketManager(ticket_lifetime_hours=24)

    def test_create_ticket(self):
        """チケット作成テスト"""
        sns_list = ['x', 'bluesky', 'mastodon']
        ticket_id = self.manager.create_ticket(sns_list)

        assert isinstance(ticket_id, str)
        assert len(ticket_id) == 36  # UUID形式: 8-4-4-4-12
        assert '-' in ticket_id

    def test_create_ticket_with_media_files(self):
        """メディアファイル付きチケット作成テスト"""
        sns_list = ['x']
        media_files = ['/tmp/image1.jpg', '/tmp/image2.png']

        ticket_id = self.manager.create_ticket(sns_list, media_files)

        assert ticket_id in self.manager.tickets
        assert self.manager.tickets[ticket_id]['media_files'] == media_files

    def test_get_status_success(self):
        """ステータス取得成功テスト"""
        sns_list = ['x', 'bluesky']
        ticket_id = self.manager.create_ticket(sns_list)

        status = self.manager.get_status(ticket_id, 'x')

        assert status is not None
        assert status['status'] == 'processing'
        assert status['message'] is None
        assert isinstance(status['updated_at'], datetime)

    def test_get_status_not_found(self):
        """チケット未検出テスト"""
        status = self.manager.get_status('invalid-ticket-id', 'x')
        assert status is None

    def test_get_status_expired(self):
        """期限切れチケットテスト"""
        ticket_id = self.manager.create_ticket(['x'])

        # チケットの有効期限を過去に設定（テスト用）
        with patch('src.web.ticket_manager.datetime') as mock_datetime:
            mock_now = datetime.now() + timedelta(hours=25)
            mock_datetime.now.return_value = mock_now

            status = self.manager.get_status(ticket_id, 'x')

        assert status is None
        # 自動クリーンアップされたことを確認
        assert ticket_id not in self.manager.tickets

    def test_update_status_success(self):
        """ステータス更新成功テスト"""
        ticket_id = self.manager.create_ticket(['x', 'bluesky'])

        result = self.manager.update_status(ticket_id, 'x', 'success', 'Posted successfully')
        assert result is True

        status = self.manager.get_status(ticket_id, 'x')
        assert status['status'] == 'success'
        assert status['message'] == 'Posted successfully'

    def test_update_status_failed(self):
        """ステータス更新失敗テスト"""
        ticket_id = self.manager.create_ticket(['x'])

        result = self.manager.update_status(ticket_id, 'x', 'failed', 'Error occurred')
        assert result is True

        status = self.manager.get_status(ticket_id, 'x')
        assert status['status'] == 'failed'
        assert status['message'] == 'Error occurred'

    def test_update_status_error(self):
        """ステータス更新エラーテスト"""
        ticket_id = self.manager.create_ticket(['x'])

        result = self.manager.update_status(ticket_id, 'x', 'error', 'Exception occurred')
        assert result is True

        status = self.manager.get_status(ticket_id, 'x')
        assert status['status'] == 'error'

    def test_update_status_invalid_status(self):
        """無効なステータス値テスト"""
        ticket_id = self.manager.create_ticket(['x'])

        with pytest.raises(ValueError) as exc_info:
            self.manager.update_status(ticket_id, 'x', 'invalid_status')

        assert 'Invalid status' in str(exc_info.value)

    def test_update_status_ticket_not_found(self):
        """チケット未検出時の更新テスト"""
        result = self.manager.update_status('invalid-ticket', 'x', 'success')
        assert result is False

    def test_update_status_sns_not_found(self):
        """SNS未検出時の更新テスト"""
        ticket_id = self.manager.create_ticket(['x'])

        result = self.manager.update_status(ticket_id, 'unknown_sns', 'success')
        assert result is False

    def test_cleanup_ticket_with_media_files(self):
        """メディアファイル削除テスト"""
        # 一時ファイルを作成
        temp_dir = tempfile.mkdtemp()
        media_file = os.path.join(temp_dir, 'test_image.jpg')
        with open(media_file, 'w') as f:
            f.write('test')

        assert os.path.exists(media_file)

        ticket_id = self.manager.create_ticket(['x'], [media_file])

        # ファイルをクリーンアップ
        self.manager._cleanup_ticket(ticket_id)

        # ファイルが削除されたことを確認
        assert not os.path.exists(media_file)
        # チケットが削除されたことを確認
        assert ticket_id not in self.manager.tickets

    def test_cleanup_ticket_without_media_files(self):
        """メディアファイルなしのクリーンアップテスト"""
        ticket_id = self.manager.create_ticket(['x'])

        self.manager._cleanup_ticket(ticket_id)

        assert ticket_id not in self.manager.tickets

    def test_cleanup_ticket_nonexistent(self):
        """存在しないチケットのクリーンアップテスト"""
        # エラーが発生しないことを確認
        self.manager._cleanup_ticket('nonexistent-ticket')

    def test_cleanup_expired_tickets(self):
        """期限切れチケット一括クリーンアップテスト"""
        # 複数のチケットを作成
        ticket_id_1 = self.manager.create_ticket(['x'])
        ticket_id_2 = self.manager.create_ticket(['bluesky'])
        ticket_id_3 = self.manager.create_ticket(['mastodon'])

        # 1つ目と3つ目のチケットを期限切れに設定
        self.manager.tickets[ticket_id_1]['expire_at'] = datetime.now() - timedelta(hours=1)
        self.manager.tickets[ticket_id_3]['expire_at'] = datetime.now() - timedelta(hours=1)

        # 期限切れチケットをクリーンアップ
        cleaned_count = self.manager.cleanup_expired_tickets()

        assert cleaned_count == 2
        assert ticket_id_1 not in self.manager.tickets
        assert ticket_id_2 in self.manager.tickets
        assert ticket_id_3 not in self.manager.tickets

    def test_multiple_sns_statuses(self):
        """複数SNSのステータス管理テスト"""
        sns_list = ['x', 'bluesky', 'mastodon']
        ticket_id = self.manager.create_ticket(sns_list)

        # 各SNSのステータスを異なる値に更新
        self.manager.update_status(ticket_id, 'x', 'success', 'X success')
        self.manager.update_status(ticket_id, 'bluesky', 'failed', 'Bluesky failed')
        self.manager.update_status(ticket_id, 'mastodon', 'processing')

        # 各ステータスが正しく保存されていることを確認
        assert self.manager.get_status(ticket_id, 'x')['status'] == 'success'
        assert self.manager.get_status(ticket_id, 'bluesky')['status'] == 'failed'
        assert self.manager.get_status(ticket_id, 'mastodon')['status'] == 'processing'

    def test_thread_safety(self):
        """スレッドセーフテスト（基本的な確認）"""
        import threading

        sns_list = ['x']
        ticket_id = self.manager.create_ticket(sns_list)
        results = []

        def update_status():
            for i in range(10):
                result = self.manager.update_status(
                    ticket_id, 'x', 'processing', f'Update {i}'
                )
                results.append(result)

        # 複数スレッドで更新
        threads = [threading.Thread(target=update_status) for _ in range(3)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        # すべての更新が成功したことを確認
        assert all(results)
        assert len(results) == 30

    def test_ticket_lifetime_configuration(self):
        """チケット有効期限設定テスト"""
        manager_12h = TicketManager(ticket_lifetime_hours=12)
        ticket_id = manager_12h.create_ticket(['x'])

        ticket = manager_12h.tickets[ticket_id]
        expected_expiry = ticket['created_at'] + timedelta(hours=12)

        # 有効期限が正しく設定されていることを確認
        assert ticket['expire_at'] == expected_expiry

    def test_status_message_persistence(self):
        """ステータスメッセージ保持テスト"""
        ticket_id = self.manager.create_ticket(['x'])
        message = 'Test message with special chars: <>&"'

        self.manager.update_status(ticket_id, 'x', 'success', message)

        status = self.manager.get_status(ticket_id, 'x')
        assert status['message'] == message
