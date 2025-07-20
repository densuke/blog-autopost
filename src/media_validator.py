#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
メディアファイルの検証機能を提供するモジュール

このモジュールは以下の機能を提供します:
- メディアファイルの形式・サイズ検証
- SNS別の投稿制限チェック
- 投稿前のバリデーション結果レポート
"""

import os
import mimetypes
from typing import List, Optional, Dict, Any
from dataclasses import dataclass
from pathlib import Path


class ValidationError(Exception):
    """メディア検証エラー"""
    pass


@dataclass
class MediaFile:
    """メディアファイル情報を格納するクラス"""
    path: str
    file_type: str  # 'image', 'video', 'audio'
    extension: str
    size: int
    mime_type: Optional[str] = None
    
    def __init__(self, file_path: str):
        """
        メディアファイルを初期化します
        
        Args:
            file_path: ファイルパス
            
        Raises:
            FileNotFoundError: ファイルが存在しない場合
            ValidationError: 未対応の形式の場合
        """
        if not os.path.exists(file_path):
            raise FileNotFoundError(f"ファイルが見つかりません: {file_path}")
        
        self.path = file_path
        self.size = os.path.getsize(file_path)
        self.extension = Path(file_path).suffix.lower()
        self.mime_type, _ = mimetypes.guess_type(file_path)
        
        # ファイル種別を判定
        self.file_type = self._determine_file_type()
        
        # 対応形式チェック
        if not self._is_supported_format():
            raise ValidationError(f"未対応のファイル形式です: {self.extension}")
    
    def _determine_file_type(self) -> str:
        """ファイル種別を判定します"""
        image_extensions = {'.jpg', '.jpeg', '.png', '.gif', '.webp'}
        video_extensions = {'.mp4', '.mov', '.webm', '.avi'}
        audio_extensions = {'.mp3', '.m4a', '.aac', '.wav', '.ogg', '.flac'}
        
        if self.extension in image_extensions:
            return 'image'
        elif self.extension in video_extensions:
            return 'video'
        elif self.extension in audio_extensions:
            return 'audio'
        else:
            return 'unknown'
    
    def _is_supported_format(self) -> bool:
        """対応形式かどうかをチェックします"""
        supported_extensions = {
            '.jpg', '.jpeg', '.png', '.gif', '.webp',  # 画像
            '.mp4', '.mov', '.webm', '.avi',           # 動画
            '.mp3', '.m4a', '.aac', '.wav', '.ogg', '.flac'  # 音声
        }
        return self.extension in supported_extensions


@dataclass 
class ValidationResult:
    """検証結果を格納するクラス"""
    is_valid: bool
    errors: List[str]
    warnings: List[str]
    converted_files: Dict[str, str]  # 変換されたファイルのマッピング
    
    def __init__(self):
        self.is_valid = True
        self.errors = []
        self.warnings = []
        self.converted_files = {}
    
    def add_error(self, message: str):
        """エラーを追加します"""
        self.errors.append(message)
        self.is_valid = False
    
    def add_warning(self, message: str):
        """警告を追加します"""
        self.warnings.append(message)
    
    def add_conversion(self, original_path: str, converted_path: str):
        """変換ファイル情報を追加します"""
        self.converted_files[original_path] = converted_path


class MediaValidator:
    """メディアファイルの検証を行うクラス"""
    
    # SNS別の制限設定
    SNS_LIMITS = {
        'x': {
            'max_images': 4,
            'max_videos': 1,
            'max_audio': 0,  # 直接は不可、変換で対応
            'allow_mixed': False,
            'max_file_size_mb': 5,  # 画像の場合
            'max_video_size_mb': 512
        },
        'bluesky': {
            'max_images': 4,
            'max_videos': 0,
            'max_audio': 0,
            'allow_mixed': False,
            'max_file_size_mb': 1
        },
        'mastodon': {
            'max_images': 4,
            'max_videos': 4,
            'max_audio': 4,
            'max_total': 4,
            'allow_mixed': True,
            'max_file_size_mb': 10,
            'max_video_size_mb': 40
        },
        'misskey': {
            'max_images': 16,
            'max_videos': 16,
            'max_audio': 16,
            'max_total': 16,
            'allow_mixed': True,
            'max_file_size_mb': 10
        }
    }
    
    def validate_files(self, file_paths: List[str]) -> List[MediaFile]:
        """
        ファイルパスリストからMediaFileリストを作成・検証します
        
        Args:
            file_paths: ファイルパスのリスト
            
        Returns:
            MediaFileのリスト
            
        Raises:
            ValidationError: ファイル検証エラー
        """
        media_files = []
        
        for file_path in file_paths:
            try:
                media_file = MediaFile(file_path)
                media_files.append(media_file)
            except (FileNotFoundError, ValidationError) as e:
                raise ValidationError(f"ファイル '{file_path}': {str(e)}")
        
        return media_files
    
    def validate_for_sns(self, media_files: List[MediaFile], sns_type: str) -> ValidationResult:
        """
        指定されたSNSに対してメディアファイルを検証します
        
        Args:
            media_files: 検証対象のMediaFileリスト
            sns_type: SNS種別 ('x', 'bluesky', 'mastodon', 'misskey')
            
        Returns:
            ValidationResult: 検証結果
        """
        result = ValidationResult()
        
        if sns_type not in self.SNS_LIMITS:
            result.add_error(f"未対応のSNS種別です: {sns_type}")
            return result
        
        limits = self.SNS_LIMITS[sns_type]
        
        # ファイル種別別にカウント
        images = [f for f in media_files if f.file_type == 'image']
        videos = [f for f in media_files if f.file_type == 'video']
        audios = [f for f in media_files if f.file_type == 'audio']
        
        # 基本制限チェック
        self._check_file_count_limits(result, images, videos, audios, limits, sns_type)
        
        # 混在制限チェック
        self._check_mixed_media_limits(result, images, videos, audios, limits, sns_type)
        
        # ファイルサイズ制限チェック
        self._check_file_size_limits(result, media_files, limits, sns_type)
        
        # SNS固有の制限チェック
        self._check_sns_specific_limits(result, images, videos, audios, sns_type)
        
        return result
    
    def _check_file_count_limits(self, result: ValidationResult, images: List[MediaFile], 
                                videos: List[MediaFile], audios: List[MediaFile], 
                                limits: Dict[str, Any], sns_type: str):
        """ファイル数制限をチェックします"""
        
        # 画像数チェック
        if len(images) > limits['max_images']:
            result.add_error(f"{sns_type.upper()} では画像は最大{limits['max_images']}枚までです（現在: {len(images)}枚）")
        
        # 動画数チェック
        if len(videos) > limits['max_videos']:
            if limits['max_videos'] == 0:
                result.add_error(f"{sns_type.upper()} では動画に対応していません")
            else:
                result.add_error(f"{sns_type.upper()} では動画は最大{limits['max_videos']}本までです（現在: {len(videos)}本）")
        
        # 音声数チェック
        if len(audios) > limits['max_audio']:
            if limits['max_audio'] == 0 and sns_type == 'x':
                # Xの場合は変換対応の警告
                for audio in audios:
                    result.add_warning(f"音声ファイル '{audio.path}' は動画形式(MP4)に変換されます")
            elif limits['max_audio'] == 0:
                result.add_error(f"{sns_type.upper()} では音声ファイルに対応していません")
            else:
                result.add_error(f"{sns_type.upper()} では音声は最大{limits['max_audio']}個までです（現在: {len(audios)}個）")
        
        # 合計数チェック（MastodonとMisskey）
        if 'max_total' in limits:
            total_files = len(images) + len(videos) + len(audios)
            if total_files > limits['max_total']:
                result.add_error(f"{sns_type.upper()} では最大{limits['max_total']}個までのファイルを添付できます（現在: {total_files}個）")
    
    def _check_mixed_media_limits(self, result: ValidationResult, images: List[MediaFile], 
                                 videos: List[MediaFile], audios: List[MediaFile], 
                                 limits: Dict[str, Any], sns_type: str):
        """混在メディア制限をチェックします"""
        
        if not limits['allow_mixed']:
            file_types = []
            if images:
                file_types.append('画像')
            if videos:
                file_types.append('動画')
            if audios and limits['max_audio'] > 0:
                file_types.append('音声')
            elif audios and sns_type != 'x':  # X以外では音声変換なし
                file_types.append('音声')
            
            if len(file_types) > 1:
                result.add_error(f"{sns_type.upper()} では{' と '.join(file_types)}を同時に投稿できません")
    
    def _check_file_size_limits(self, result: ValidationResult, media_files: List[MediaFile], 
                               limits: Dict[str, Any], sns_type: str):
        """ファイルサイズ制限をチェックします"""
        
        max_size_mb = limits.get('max_file_size_mb', 10)
        max_video_size_mb = limits.get('max_video_size_mb', max_size_mb)
        
        for media_file in media_files:
            size_mb = media_file.size / (1024 * 1024)
            
            if media_file.file_type == 'video':
                if size_mb > max_video_size_mb:
                    result.add_error(f"動画ファイル '{media_file.path}' のサイズが制限を超えています "
                                   f"({size_mb:.1f}MB > {max_video_size_mb}MB)")
            else:
                if size_mb > max_size_mb:
                    result.add_error(f"ファイル '{media_file.path}' のサイズが制限を超えています "
                                   f"({size_mb:.1f}MB > {max_size_mb}MB)")
    
    def _check_sns_specific_limits(self, result: ValidationResult, images: List[MediaFile], 
                                  videos: List[MediaFile], audios: List[MediaFile], sns_type: str):
        """SNS固有の制限をチェックします"""
        
        if sns_type == 'x':
            # Xでは画像と動画の同時投稿不可
            if images and videos:
                result.add_error("X では画像と動画を同時に投稿できません")
            
            # 動画は1本まで
            if len(videos) > 1:
                result.add_error("X では動画は1本までです")
        
        elif sns_type == 'bluesky':
            # Blueskyは画像のみ
            if videos:
                result.add_error("Bluesky では動画に対応していません")
            if audios:
                result.add_error("Bluesky では音声ファイルに対応していません")


def validate_media_for_posting(file_paths: List[str], target_sns: List[str]) -> Dict[str, ValidationResult]:
    """
    投稿用のメディア検証を行います
    
    Args:
        file_paths: メディアファイルのパスリスト
        target_sns: 対象SNSのリスト
        
    Returns:
        SNS別の検証結果辞書
        
    Raises:
        ValidationError: ファイル検証エラー
    """
    validator = MediaValidator()
    
    # MediaFileリストを作成
    media_files = validator.validate_files(file_paths)
    
    # SNS別に検証
    results = {}
    for sns_type in target_sns:
        results[sns_type] = validator.validate_for_sns(media_files, sns_type)
    
    return results