#!/usr/bin/env python
# -*- coding: utf-8 -*-

import io
import os
import tempfile
from typing import Any, Dict

from PIL import Image


class ImageResizer:
    """
    画像リサイズ処理を行うクラス
    SNS別のサイズ制限に応じて画像を最適化する
    """

    # SNS別の画像制限設定
    SNS_LIMITS = {
        'bluesky': {
            'max_file_size': 1024 * 1024,  # 1MB
            'max_width': 2000,
            'max_height': 2000,
            'quality': 85
        },
        'x': {
            'max_file_size': 5 * 1024 * 1024,  # 5MB
            'max_width': 4096,
            'max_height': 4096,
            'quality': 85
        },
        'mastodon': {
            'max_file_size': 10 * 1024 * 1024,  # 10MB（一般的な設定）
            'max_width': 1920,
            'max_height': 1920,
            'quality': 85
        },
        'misskey': {
            'max_file_size': 10 * 1024 * 1024,  # 10MB（一般的な設定）
            'max_width': 2048,
            'max_height': 2048,
            'quality': 85
        }
    }

    def __init__(self, debug: bool = False):
        self.debug = debug

    def _debug_print(self, message: str):
        """デバッグメッセージを出力"""
        if self.debug:
            print(f"[DEBUG] ImageResizer: {message}")

    def resize_image_data(self, image_data: bytes, sns_type: str = 'bluesky') -> bytes:
        """
        画像データをSNS制限に合わせてリサイズする
        
        Args:
            image_data: 元画像のバイトデータ
            sns_type: SNSの種類（bluesky, x, mastodon, misskey）
            
        Returns:
            bytes: リサイズ後の画像データ
        """
        try:
            # SNS設定を取得
            limits = self.SNS_LIMITS.get(sns_type, self.SNS_LIMITS['bluesky'])
            max_size = limits['max_file_size']
            max_width = limits['max_width']
            max_height = limits['max_height']
            quality = limits['quality']

            self._debug_print(f"開始: {sns_type}, 元サイズ: {len(image_data)} bytes")

            # 元画像がサイズ制限内の場合はそのまま返す
            if len(image_data) <= max_size:
                image = Image.open(io.BytesIO(image_data))
                if image.width <= max_width and image.height <= max_height:
                    self._debug_print("リサイズ不要")
                    return image_data

            # 画像を開く
            image = Image.open(io.BytesIO(image_data))
            original_format = image.format

            # RGBAモードの場合はRGBに変換（JPEGサポートのため）
            if image.mode in ('RGBA', 'LA'):
                background = Image.new('RGB', image.size, (255, 255, 255))
                if image.mode == 'RGBA':
                    background.paste(image, mask=image.split()[-1])
                else:
                    background.paste(image)
                image = background
            elif image.mode != 'RGB':
                image = image.convert('RGB')

            self._debug_print(f"元画像: {image.width}x{image.height}, 形式: {original_format}")

            # アスペクト比を保持してリサイズ
            if image.width > max_width or image.height > max_height:
                image.thumbnail((max_width, max_height), Image.Resampling.LANCZOS)
                self._debug_print(f"サイズ調整後: {image.width}x{image.height}")

            # 品質を調整してファイルサイズを制限内に収める
            for current_quality in range(quality, 30, -5):  # 品質を段階的に下げる
                output = io.BytesIO()
                image.save(output, format='JPEG', quality=current_quality, optimize=True)
                result_data = output.getvalue()

                self._debug_print(f"品質{current_quality}: {len(result_data)} bytes")

                if len(result_data) <= max_size:
                    self._debug_print(f"完了: {len(result_data)} bytes (品質: {current_quality})")
                    return result_data

            # 最低品質でも制限を超える場合は、さらにサイズを縮小
            scale_factor = 0.9
            while len(result_data) > max_size and scale_factor > 0.5:
                new_width = int(image.width * scale_factor)
                new_height = int(image.height * scale_factor)
                resized = image.resize((new_width, new_height), Image.Resampling.LANCZOS)

                output = io.BytesIO()
                resized.save(output, format='JPEG', quality=30, optimize=True)
                result_data = output.getvalue()

                self._debug_print(f"追加縮小 {scale_factor:.1f}: {new_width}x{new_height}, {len(result_data)} bytes")

                if len(result_data) <= max_size:
                    break

                scale_factor -= 0.1

            self._debug_print(f"最終: {len(result_data)} bytes")
            return result_data

        except Exception as e:
            self._debug_print(f"リサイズエラー: {e}")
            # エラーの場合は元画像を返す
            return image_data

    def resize_image_file(self, file_path: str, sns_type: str = 'bluesky') -> str:
        """
        画像ファイルをSNS制限に合わせてリサイズし、一時ファイルとして保存
        
        Args:
            file_path: 元画像ファイルのパス
            sns_type: SNSの種類
            
        Returns:
            str: リサイズ後の一時ファイルパス
        """
        try:
            with open(file_path, 'rb') as f:
                original_data = f.read()

            resized_data = self.resize_image_data(original_data, sns_type)

            # 一時ファイルとして保存
            with tempfile.NamedTemporaryFile(delete=False, suffix='.jpg') as temp_file:
                temp_file.write(resized_data)
                temp_path = temp_file.name

            self._debug_print(f"一時ファイル作成: {temp_path}")
            return temp_path

        except Exception as e:
            self._debug_print(f"ファイルリサイズエラー: {e}")
            return file_path  # エラーの場合は元ファイルを返す

    def get_sns_limits(self, sns_type: str) -> Dict[str, Any]:
        """
        指定したSNSの制限情報を取得
        
        Args:
            sns_type: SNSの種類
            
        Returns:
            Dict[str, Any]: 制限情報
        """
        return self.SNS_LIMITS.get(sns_type, self.SNS_LIMITS['bluesky'])

    @staticmethod
    def cleanup_temp_file(file_path: str):
        """
        一時ファイルを削除
        
        Args:
            file_path: 削除する一時ファイルのパス
        """
        try:
            if file_path and os.path.exists(file_path) and file_path.startswith(tempfile.gettempdir()):
                os.unlink(file_path)
        except Exception:
            pass  # 削除エラーは無視


def create_image_resizer(debug: bool = False) -> ImageResizer:
    """
    ImageResizerインスタンスを作成
    
    Args:
        debug: デバッグモードの有効/無効
        
    Returns:
        ImageResizer: ImageResizerインスタンス
    """
    return ImageResizer(debug=debug)
