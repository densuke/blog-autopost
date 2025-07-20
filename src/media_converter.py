#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
メディアファイルの変換機能を提供するモジュール

このモジュールは以下の機能を提供します:
- m4a音声ファイルのMP4動画変換
- ffmpegを使った音声・動画変換
- 変換プロセスの進行状況管理
"""

import os
import subprocess
import tempfile
import shutil
from typing import Optional, Dict, Any
from pathlib import Path


class ConversionError(Exception):
    """メディア変換エラー"""
    pass


class MediaConverter:
    """メディアファイルの変換を行うクラス"""
    
    def __init__(self, ffmpeg_path: str = "ffmpeg"):
        """
        MediaConverterを初期化します
        
        Args:
            ffmpeg_path: ffmpegコマンドのパス（デフォルト: "ffmpeg"）
        """
        self.ffmpeg_path = ffmpeg_path
        self._check_ffmpeg_available()
    
    def _check_ffmpeg_available(self):
        """ffmpegが利用可能かチェックします"""
        try:
            result = subprocess.run(
                [self.ffmpeg_path, "-version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            if result.returncode != 0:
                raise ConversionError("ffmpegが正常に動作しません")
        except FileNotFoundError:
            raise ConversionError(
                "ffmpegが見つかりません。ffmpegをインストールしてPATHに追加してください。\n"
                "インストール方法: https://ffmpeg.org/download.html"
            )
        except subprocess.TimeoutExpired:
            raise ConversionError("ffmpegの応答がタイムアウトしました")
    
    def convert_m4a_to_mp4(self, input_path: str, output_dir: Optional[str] = None) -> str:
        """
        m4a音声ファイルを無音黒画面付きMP4動画に変換します
        
        Args:
            input_path: 入力m4aファイルのパス
            output_dir: 出力ディレクトリ（Noneの場合は一時ディレクトリ）
            
        Returns:
            変換後のMP4ファイルパス
            
        Raises:
            FileNotFoundError: 入力ファイルが存在しない場合
            ConversionError: 変換に失敗した場合
        """
        if not os.path.exists(input_path):
            raise FileNotFoundError(f"入力ファイルが見つかりません: {input_path}")
        
        # 出力ファイルパスを決定
        input_stem = Path(input_path).stem
        if output_dir is None:
            output_dir = tempfile.gettempdir()
        output_path = os.path.join(output_dir, f"{input_stem}_converted.mp4")
        
        # ffmpegコマンドを構築
        cmd = [
            self.ffmpeg_path,
            "-i", input_path,                    # 音声入力
            "-f", "lavfi",                       # ラベルフィルター
            "-i", "color=black:size=320x240:rate=1",  # 黒画面生成（低解像度、低フレームレート）
            "-c:v", "libx264",                   # 動画コーデック
            "-c:a", "copy",                      # 音声コーデック（コピー）
            "-shortest",                         # 短い方に合わせる
            "-y",                                # 上書き確認をスキップ
            output_path
        ]
        
        try:
            # ffmpeg実行
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=300  # 5分でタイムアウト
            )
            
            if result.returncode != 0:
                error_msg = f"ffmpeg変換エラー: {result.stderr}"
                raise ConversionError(error_msg)
            
            # 出力ファイルの存在確認
            if not os.path.exists(output_path):
                raise ConversionError("変換は成功しましたが、出力ファイルが見つかりません")
            
            return output_path
            
        except subprocess.TimeoutExpired:
            raise ConversionError("ffmpeg変換がタイムアウトしました（5分以上）")
        except Exception as e:
            raise ConversionError(f"変換中に予期しないエラーが発生しました: {str(e)}")
    
    def convert_audio_to_mp4_with_image(self, input_path: str, image_path: str, 
                                       output_dir: Optional[str] = None) -> str:
        """
        音声ファイルと静止画を組み合わせてMP4動画を作成します
        
        Args:
            input_path: 入力音声ファイルのパス
            image_path: 静止画ファイルのパス
            output_dir: 出力ディレクトリ（Noneの場合は一時ディレクトリ）
            
        Returns:
            変換後のMP4ファイルパス
            
        Raises:
            FileNotFoundError: 入力ファイルが存在しない場合
            ConversionError: 変換に失敗した場合
        """
        if not os.path.exists(input_path):
            raise FileNotFoundError(f"音声ファイルが見つかりません: {input_path}")
        if not os.path.exists(image_path):
            raise FileNotFoundError(f"画像ファイルが見つかりません: {image_path}")
        
        # 出力ファイルパスを決定
        input_stem = Path(input_path).stem
        if output_dir is None:
            output_dir = tempfile.gettempdir()
        output_path = os.path.join(output_dir, f"{input_stem}_with_image.mp4")
        
        # ffmpegコマンドを構築
        cmd = [
            self.ffmpeg_path,
            "-loop", "1",                        # 画像をループ
            "-i", image_path,                    # 画像入力
            "-i", input_path,                    # 音声入力
            "-c:v", "libx264",                   # 動画コーデック
            "-c:a", "aac",                       # 音声コーデック
            "-b:a", "192k",                      # 音声ビットレート
            "-pix_fmt", "yuv420p",               # ピクセルフォーマット（互換性向上）
            "-shortest",                         # 短い方に合わせる
            "-y",                                # 上書き確認をスキップ
            output_path
        ]
        
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=300
            )
            
            if result.returncode != 0:
                error_msg = f"ffmpeg変換エラー: {result.stderr}"
                raise ConversionError(error_msg)
            
            if not os.path.exists(output_path):
                raise ConversionError("変換は成功しましたが、出力ファイルが見つかりません")
            
            return output_path
            
        except subprocess.TimeoutExpired:
            raise ConversionError("ffmpeg変換がタイムアウトしました（5分以上）")
        except Exception as e:
            raise ConversionError(f"変換中に予期しないエラーが発生しました: {str(e)}")
    
    def get_media_info(self, file_path: str) -> Dict[str, Any]:
        """
        メディアファイルの情報を取得します
        
        Args:
            file_path: メディアファイルのパス
            
        Returns:
            メディア情報の辞書
            
        Raises:
            FileNotFoundError: ファイルが存在しない場合
            ConversionError: 情報取得に失敗した場合
        """
        if not os.path.exists(file_path):
            raise FileNotFoundError(f"ファイルが見つかりません: {file_path}")
        
        cmd = [
            self.ffmpeg_path,
            "-i", file_path,
            "-f", "null", "-"
        ]
        
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=30
            )
            
            # ffmpegは情報をstderrに出力する
            output = result.stderr
            
            info = {
                'duration': self._extract_duration(output),
                'video_codec': self._extract_video_codec(output),
                'audio_codec': self._extract_audio_codec(output),
                'resolution': self._extract_resolution(output),
                'file_size': os.path.getsize(file_path)
            }
            
            return info
            
        except subprocess.TimeoutExpired:
            raise ConversionError("メディア情報の取得がタイムアウトしました")
        except Exception as e:
            raise ConversionError(f"メディア情報の取得中にエラーが発生しました: {str(e)}")
    
    def _extract_duration(self, ffmpeg_output: str) -> Optional[str]:
        """ffmpeg出力から動画の長さを抽出します"""
        import re
        duration_match = re.search(r'Duration: (\d{2}:\d{2}:\d{2}\.\d{2})', ffmpeg_output)
        return duration_match.group(1) if duration_match else None
    
    def _extract_video_codec(self, ffmpeg_output: str) -> Optional[str]:
        """ffmpeg出力から動画コーデックを抽出します"""
        import re
        video_match = re.search(r'Video: (\w+)', ffmpeg_output)
        return video_match.group(1) if video_match else None
    
    def _extract_audio_codec(self, ffmpeg_output: str) -> Optional[str]:
        """ffmpeg出力から音声コーデックを抽出します"""
        import re
        audio_match = re.search(r'Audio: (\w+)', ffmpeg_output)
        return audio_match.group(1) if audio_match else None
    
    def _extract_resolution(self, ffmpeg_output: str) -> Optional[str]:
        """ffmpeg出力から解像度を抽出します"""
        import re
        resolution_match = re.search(r'(\d{3,4}x\d{3,4})', ffmpeg_output)
        return resolution_match.group(1) if resolution_match else None
    
    def cleanup_temp_files(self, file_paths: list):
        """一時ファイルをクリーンアップします"""
        for file_path in file_paths:
            try:
                if os.path.exists(file_path) and tempfile.gettempdir() in file_path:
                    os.remove(file_path)
            except OSError:
                pass  # ファイル削除失敗は無視


# コンバーターのファクトリー関数
def create_media_converter(ffmpeg_path: str = "ffmpeg") -> MediaConverter:
    """
    MediaConverterインスタンスを作成します
    
    Args:
        ffmpeg_path: ffmpegコマンドのパス
        
    Returns:
        MediaConverterインスタンス
        
    Raises:
        ConversionError: ffmpegが利用できない場合
    """
    return MediaConverter(ffmpeg_path)


# SNS向けの便利関数
def convert_audio_for_x(input_path: str) -> str:
    """
    X投稿用に音声ファイルをMP4に変換します
    
    Args:
        input_path: 入力音声ファイルのパス
        
    Returns:
        変換後のMP4ファイルパス
        
    Raises:
        ConversionError: 変換に失敗した場合
    """
    converter = create_media_converter()
    return converter.convert_m4a_to_mp4(input_path)


def is_ffmpeg_available() -> bool:
    """
    ffmpegが利用可能かチェックします
    
    Returns:
        ffmpegが利用可能な場合True
    """
    try:
        create_media_converter()
        return True
    except ConversionError:
        return False