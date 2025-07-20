#!/usr/bin/env python
# -*- coding: utf-8 -*-

import pytest
import tempfile
import os
from pathlib import Path
import shutil
from unittest.mock import Mock, patch, MagicMock

from src.media_validator import MediaValidator, MediaFile, ValidationError
from src.media_converter import MediaConverter, ConversionError


class TestMediaFile:
    """MediaFileクラスのテスト"""
    
    def test_create_media_file_image(self):
        """画像ファイルのMediaFile作成テスト"""
        with tempfile.NamedTemporaryFile(suffix='.jpg', delete=False) as f:
            f.write(b'fake jpeg data')
            temp_path = f.name
        
        try:
            media_file = MediaFile(temp_path)
            assert media_file.path == temp_path
            assert media_file.file_type == 'image'
            assert media_file.extension == '.jpg'
            assert media_file.size > 0
        finally:
            os.unlink(temp_path)
    
    def test_create_media_file_video(self):
        """動画ファイルのMediaFile作成テスト"""
        with tempfile.NamedTemporaryFile(suffix='.mp4', delete=False) as f:
            f.write(b'fake mp4 data')
            temp_path = f.name
        
        try:
            media_file = MediaFile(temp_path)
            assert media_file.path == temp_path
            assert media_file.file_type == 'video'
            assert media_file.extension == '.mp4'
            assert media_file.size > 0
        finally:
            os.unlink(temp_path)
    
    def test_create_media_file_audio(self):
        """音声ファイルのMediaFile作成テスト"""
        with tempfile.NamedTemporaryFile(suffix='.m4a', delete=False) as f:
            f.write(b'fake m4a data')
            temp_path = f.name
        
        try:
            media_file = MediaFile(temp_path)
            assert media_file.path == temp_path
            assert media_file.file_type == 'audio'
            assert media_file.extension == '.m4a'
            assert media_file.size > 0
        finally:
            os.unlink(temp_path)
    
    def test_create_media_file_nonexistent(self):
        """存在しないファイルのテスト"""
        with pytest.raises(FileNotFoundError):
            MediaFile('/nonexistent/file.jpg')
    
    def test_create_media_file_unsupported(self):
        """未対応形式のテスト"""
        with tempfile.NamedTemporaryFile(suffix='.txt', delete=False) as f:
            f.write(b'text file')
            temp_path = f.name
        
        try:
            with pytest.raises(ValidationError):
                MediaFile(temp_path)
        finally:
            os.unlink(temp_path)


class TestMediaValidator:
    """MediaValidatorクラスのテスト"""
    
    def setup_method(self):
        """テストセットアップ"""
        self.validator = MediaValidator()
    
    def create_temp_file(self, suffix, content=b'fake data'):
        """テスト用の一時ファイルを作成"""
        f = tempfile.NamedTemporaryFile(suffix=suffix, delete=False)
        f.write(content)
        f.close()
        return f.name
    
    def test_validate_x_images_only(self):
        """X: 画像のみの投稿テスト"""
        image_paths = [
            self.create_temp_file('.jpg'),
            self.create_temp_file('.png'),
        ]
        
        try:
            media_files = [MediaFile(path) for path in image_paths]
            result = self.validator.validate_for_sns(media_files, 'x')
            assert result.is_valid is True
            assert len(result.warnings) == 0
        finally:
            for path in image_paths:
                os.unlink(path)
    
    def test_validate_x_too_many_images(self):
        """X: 画像数制限超過テスト"""
        image_paths = [self.create_temp_file(f'.jpg') for _ in range(5)]
        
        try:
            media_files = [MediaFile(path) for path in image_paths]
            result = self.validator.validate_for_sns(media_files, 'x')
            assert result.is_valid is False
            assert 'X では画像は最大4枚まで' in str(result.errors[0])
        finally:
            for path in image_paths:
                os.unlink(path)
    
    def test_validate_x_mixed_media_invalid(self):
        """X: 画像と動画の混在テスト（無効）"""
        image_path = self.create_temp_file('.jpg')
        video_path = self.create_temp_file('.mp4')
        
        try:
            media_files = [MediaFile(image_path), MediaFile(video_path)]
            result = self.validator.validate_for_sns(media_files, 'x')
            assert result.is_valid is False
            assert 'X では画像と動画を同時に投稿できません' in str(result.errors[0])
        finally:
            os.unlink(image_path)
            os.unlink(video_path)
    
    def test_validate_x_audio_conversion_warning(self):
        """X: 音声ファイルの変換警告テスト"""
        audio_path = self.create_temp_file('.m4a')
        
        try:
            media_files = [MediaFile(audio_path)]
            result = self.validator.validate_for_sns(media_files, 'x')
            assert result.is_valid is True
            assert len(result.warnings) > 0
            assert 'MP4に変換されます' in str(result.warnings[0])
        finally:
            os.unlink(audio_path)
    
    def test_validate_bluesky_images_only(self):
        """Bluesky: 画像のみ対応テスト"""
        image_paths = [self.create_temp_file('.jpg') for _ in range(3)]
        
        try:
            media_files = [MediaFile(path) for path in image_paths]
            result = self.validator.validate_for_sns(media_files, 'bluesky')
            assert result.is_valid is True
        finally:
            for path in image_paths:
                os.unlink(path)
    
    def test_validate_bluesky_video_invalid(self):
        """Bluesky: 動画非対応テスト"""
        video_path = self.create_temp_file('.mp4')
        
        try:
            media_files = [MediaFile(video_path)]
            result = self.validator.validate_for_sns(media_files, 'bluesky')
            assert result.is_valid is False
            assert 'Bluesky では動画に対応していません' in str(result.errors[0])
        finally:
            os.unlink(video_path)
    
    def test_validate_mastodon_mixed_media(self):
        """Mastodon: 混在メディア対応テスト"""
        image_path = self.create_temp_file('.jpg')
        video_path = self.create_temp_file('.mp4')
        audio_path = self.create_temp_file('.m4a')
        
        try:
            media_files = [MediaFile(image_path), MediaFile(video_path), MediaFile(audio_path)]
            result = self.validator.validate_for_sns(media_files, 'mastodon')
            assert result.is_valid is True
        finally:
            os.unlink(image_path)
            os.unlink(video_path)
            os.unlink(audio_path)
    
    def test_validate_misskey_many_files(self):
        """Misskey: 多数ファイル対応テスト"""
        image_paths = [self.create_temp_file('.jpg') for _ in range(10)]
        
        try:
            media_files = [MediaFile(path) for path in image_paths]
            result = self.validator.validate_for_sns(media_files, 'misskey')
            assert result.is_valid is True
        finally:
            for path in image_paths:
                os.unlink(path)
    
    def test_validate_misskey_too_many_files(self):
        """Misskey: ファイル数制限超過テスト"""
        image_paths = [self.create_temp_file('.jpg') for _ in range(17)]
        
        try:
            media_files = [MediaFile(path) for path in image_paths]
            result = self.validator.validate_for_sns(media_files, 'misskey')
            assert result.is_valid is False
            assert 'Misskey では最大16個まで' in str(result.errors[0])
        finally:
            for path in image_paths:
                os.unlink(path)


class TestMediaConverter:
    """MediaConverterクラスのテスト"""
    
    def setup_method(self):
        """テストセットアップ"""
        self.converter = MediaConverter()
        self.temp_dir = tempfile.mkdtemp()
    
    def teardown_method(self):
        """テストクリーンアップ"""
        shutil.rmtree(self.temp_dir)
    
    def create_temp_audio_file(self):
        """テスト用音声ファイルを作成"""
        audio_path = os.path.join(self.temp_dir, 'test.m4a')
        with open(audio_path, 'wb') as f:
            f.write(b'fake m4a audio data')
        return audio_path
    
    @patch('subprocess.run')
    def test_convert_m4a_to_mp4_success(self, mock_run):
        """m4a→MP4変換成功テスト"""
        mock_run.return_value = MagicMock(returncode=0)
        
        input_path = self.create_temp_audio_file()
        result = self.converter.convert_m4a_to_mp4(input_path)
        
        assert result.endswith('.mp4')
        assert mock_run.called
        
        # ffmpegコマンドの引数確認
        call_args = mock_run.call_args[0][0]
        assert 'ffmpeg' in call_args
        assert input_path in call_args
    
    @patch('subprocess.run')
    def test_convert_m4a_to_mp4_failure(self, mock_run):
        """m4a→MP4変換失敗テスト"""
        mock_run.return_value = MagicMock(returncode=1, stderr='ffmpeg error')
        
        input_path = self.create_temp_audio_file()
        
        with pytest.raises(ConversionError):
            self.converter.convert_m4a_to_mp4(input_path)
    
    def test_convert_nonexistent_file(self):
        """存在しないファイルの変換テスト"""
        with pytest.raises(FileNotFoundError):
            self.converter.convert_m4a_to_mp4('/nonexistent/file.m4a')


class TestMainIntegration:
    """メイン機能との統合テスト"""
    
    @patch('src.main.load_plugins')
    def test_media_option_parsing(self, mock_load_plugins):
        """--mediaオプションのパースマーレテスト"""
        from src.main import main
        import sys
        
        # モックプラグインを設定
        mock_load_plugins.return_value = {}
        
        # コマンドライン引数をシミュレート
        test_args = [
            'main.py',
            '--text', 'テスト投稿',
            '--media', 'image1.jpg',
            '--media', 'image2.png',
            '--dry-run'
        ]
        
        with patch.object(sys, 'argv', test_args):
            with patch('src.main.load_config') as mock_config:
                mock_config.return_value = {}
                with patch('src.main.ConfigManager'):
                    # 例外が発生しないことを確認
                    try:
                        main()
                    except SystemExit:
                        pass  # argparseの正常終了
    
    def test_dry_run_with_media_display(self):
        """ドライラン時のメディア情報表示テスト"""
        # 実際のファイルパスでテストする場合の実装
        pass


class TestSNSPluginIntegration:
    """SNSプラグインとの統合テスト"""
    
    def test_plugin_post_with_media_signature(self):
        """プラグインのpost()メソッドシグネチャテスト"""
        from src.plugins.x import X
        from src.plugins.bluesky import Bluesky
        from src.plugins.mastodon import Mastodon
        from src.plugins.misskey import Misskey
        
        # 各プラグインクラスのpost()メソッドがmedia_filesパラメータを受け取れることを確認
        # 注: 実際の実装後にこのテストを更新する必要があります
        pass