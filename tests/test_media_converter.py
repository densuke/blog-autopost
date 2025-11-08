#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
media_converterモジュールのテスト
"""

import os
import tempfile
from unittest.mock import Mock, patch

import pytest

from src.media_converter import (
    ConversionError,
    MediaConverter,
    create_media_converter,
    convert_audio_for_x,
    is_ffmpeg_available,
)


class TestMediaConverter:
    """MediaConverterクラスのテスト"""

    def test_init_with_default_ffmpeg_path(self):
        """デフォルトのffmpegパスで初期化できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            assert converter.ffmpeg_path == "ffmpeg"

    def test_init_with_custom_ffmpeg_path(self):
        """カスタムffmpegパスで初期化できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter(ffmpeg_path="/usr/bin/ffmpeg")
            assert converter.ffmpeg_path == "/usr/bin/ffmpeg"

    def test_check_ffmpeg_available_success(self):
        """ffmpegが利用可能な場合、正常に初期化できること"""
        mock_result = Mock()
        mock_result.returncode = 0
        
        with patch("subprocess.run", return_value=mock_result):
            converter = MediaConverter()
            assert converter is not None

    def test_check_ffmpeg_available_not_found(self):
        """ffmpegが見つからない場合、ConversionErrorが発生すること"""
        with patch("subprocess.run", side_effect=FileNotFoundError):
            with pytest.raises(ConversionError, match="ffmpegが見つかりません"):
                MediaConverter()

    def test_check_ffmpeg_available_not_working(self):
        """ffmpegが正常に動作しない場合、ConversionErrorが発生すること"""
        mock_result = Mock()
        mock_result.returncode = 1
        
        with patch("subprocess.run", return_value=mock_result):
            with pytest.raises(ConversionError, match="ffmpegが正常に動作しません"):
                MediaConverter()

    def test_check_ffmpeg_available_timeout(self):
        """ffmpegがタイムアウトした場合、ConversionErrorが発生すること"""
        import subprocess
        with patch("subprocess.run", side_effect=subprocess.TimeoutExpired("ffmpeg", 10)):
            with pytest.raises(ConversionError, match="タイムアウト"):
                MediaConverter()

    def test_convert_m4a_to_mp4_file_not_found(self):
        """入力ファイルが存在しない場合、FileNotFoundErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            with pytest.raises(FileNotFoundError, match="入力ファイルが見つかりません"):
                converter.convert_m4a_to_mp4("/nonexistent/file.m4a")

    def test_convert_m4a_to_mp4_success(self):
        """m4aからMP4への変換が成功すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            # 一時ファイルを作成
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_input:
                input_path = tmp_input.name
            
            try:
                mock_result = Mock()
                mock_result.returncode = 0
                mock_result.stderr = ""
                
                with patch("subprocess.run", return_value=mock_result):
                    with patch("os.path.exists") as mock_exists:
                        # 最初の呼び出しは入力ファイルチェック、2回目は出力ファイルチェック
                        mock_exists.side_effect = [True, True]
                        
                        output_path = converter.convert_m4a_to_mp4(input_path)
                        assert output_path.endswith("_converted.mp4")
            finally:
                # クリーンアップ
                if os.path.exists(input_path):
                    os.remove(input_path)

    def test_convert_m4a_to_mp4_with_output_dir(self):
        """出力ディレクトリを指定してMP4変換ができること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.TemporaryDirectory() as output_dir:
                with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_input:
                    input_path = tmp_input.name
                
                try:
                    mock_result = Mock()
                    mock_result.returncode = 0
                    mock_result.stderr = ""
                    
                    with patch("subprocess.run", return_value=mock_result):
                        with patch("os.path.exists") as mock_exists:
                            mock_exists.side_effect = [True, True]
                            
                            output_path = converter.convert_m4a_to_mp4(input_path, output_dir)
                            assert output_dir in output_path
                finally:
                    if os.path.exists(input_path):
                        os.remove(input_path)

    def test_convert_m4a_to_mp4_ffmpeg_error(self):
        """ffmpegがエラーを返した場合、ConversionErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_input:
                input_path = tmp_input.name
            
            try:
                mock_result = Mock()
                mock_result.returncode = 1
                mock_result.stderr = "ffmpeg error message"
                
                with patch("subprocess.run", return_value=mock_result):
                    with patch("os.path.exists", return_value=True):
                        with pytest.raises(ConversionError, match="ffmpeg変換エラー"):
                            converter.convert_m4a_to_mp4(input_path)
            finally:
                if os.path.exists(input_path):
                    os.remove(input_path)

    def test_convert_m4a_to_mp4_output_not_found(self):
        """出力ファイルが見つからない場合、ConversionErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_input:
                input_path = tmp_input.name
            
            try:
                mock_result = Mock()
                mock_result.returncode = 0
                mock_result.stderr = ""
                
                with patch("subprocess.run", return_value=mock_result):
                    with patch("os.path.exists") as mock_exists:
                        # 入力ファイルは存在、出力ファイルは存在しない
                        mock_exists.side_effect = [True, False]
                        
                        with pytest.raises(ConversionError, match="出力ファイルが見つかりません"):
                            converter.convert_m4a_to_mp4(input_path)
            finally:
                if os.path.exists(input_path):
                    os.remove(input_path)

    def test_convert_m4a_to_mp4_timeout(self):
        """ffmpeg実行がタイムアウトした場合、ConversionErrorが発生すること"""
        import subprocess
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_input:
                input_path = tmp_input.name
            
            try:
                with patch("subprocess.run", side_effect=subprocess.TimeoutExpired("ffmpeg", 300)):
                    with patch("os.path.exists", return_value=True):
                        with pytest.raises(ConversionError, match="タイムアウト"):
                            converter.convert_m4a_to_mp4(input_path)
            finally:
                if os.path.exists(input_path):
                    os.remove(input_path)

    def test_convert_m4a_to_mp4_unexpected_error(self):
        """予期しないエラーが発生した場合、ConversionErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_input:
                input_path = tmp_input.name
            
            try:
                with patch("subprocess.run", side_effect=RuntimeError("unexpected error")):
                    with patch("os.path.exists", return_value=True):
                        with pytest.raises(ConversionError, match="予期しないエラー"):
                            converter.convert_m4a_to_mp4(input_path)
            finally:
                if os.path.exists(input_path):
                    os.remove(input_path)

    def test_convert_audio_to_mp4_with_image_success(self):
        """音声と画像からMP4を作成できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_audio:
                audio_path = tmp_audio.name
            with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_image:
                image_path = tmp_image.name
            
            try:
                mock_result = Mock()
                mock_result.returncode = 0
                mock_result.stderr = ""
                
                with patch("subprocess.run", return_value=mock_result):
                    with patch("os.path.exists") as mock_exists:
                        # 音声、画像、出力ファイル全て存在
                        mock_exists.side_effect = [True, True, True]
                        
                        output_path = converter.convert_audio_to_mp4_with_image(audio_path, image_path)
                        assert output_path.endswith("_with_image.mp4")
            finally:
                for path in [audio_path, image_path]:
                    if os.path.exists(path):
                        os.remove(path)

    def test_convert_audio_to_mp4_with_image_audio_not_found(self):
        """音声ファイルが存在しない場合、FileNotFoundErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            with pytest.raises(FileNotFoundError, match="音声ファイルが見つかりません"):
                converter.convert_audio_to_mp4_with_image("/nonexistent/audio.m4a", "/some/image.jpg")

    def test_convert_audio_to_mp4_with_image_image_not_found(self):
        """画像ファイルが存在しない場合、FileNotFoundErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_audio:
                audio_path = tmp_audio.name
            
            try:
                with patch("os.path.exists") as mock_exists:
                    # 音声ファイルは存在、画像ファイルは存在しない
                    mock_exists.side_effect = [True, False]
                    
                    with pytest.raises(FileNotFoundError, match="画像ファイルが見つかりません"):
                        converter.convert_audio_to_mp4_with_image(audio_path, "/nonexistent/image.jpg")
            finally:
                if os.path.exists(audio_path):
                    os.remove(audio_path)

    def test_convert_audio_to_mp4_with_image_with_output_dir(self):
        """出力ディレクトリを指定して音声と画像からMP4を作成できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.TemporaryDirectory() as output_dir:
                with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_audio:
                    audio_path = tmp_audio.name
                with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_image:
                    image_path = tmp_image.name
                
                try:
                    mock_result = Mock()
                    mock_result.returncode = 0
                    mock_result.stderr = ""
                    
                    with patch("subprocess.run", return_value=mock_result):
                        with patch("os.path.exists") as mock_exists:
                            mock_exists.side_effect = [True, True, True]
                            
                            output_path = converter.convert_audio_to_mp4_with_image(
                                audio_path, image_path, output_dir
                            )
                            assert output_dir in output_path
                finally:
                    for path in [audio_path, image_path]:
                        if os.path.exists(path):
                            os.remove(path)

    def test_convert_audio_to_mp4_with_image_ffmpeg_error(self):
        """ffmpegがエラーを返した場合、ConversionErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_audio:
                audio_path = tmp_audio.name
            with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_image:
                image_path = tmp_image.name
            
            try:
                mock_result = Mock()
                mock_result.returncode = 1
                mock_result.stderr = "ffmpeg error"
                
                with patch("subprocess.run", return_value=mock_result):
                    with patch("os.path.exists") as mock_exists:
                        mock_exists.side_effect = [True, True]
                        
                        with pytest.raises(ConversionError, match="ffmpeg変換エラー"):
                            converter.convert_audio_to_mp4_with_image(audio_path, image_path)
            finally:
                for path in [audio_path, image_path]:
                    if os.path.exists(path):
                        os.remove(path)

    def test_convert_audio_to_mp4_with_image_output_not_found(self):
        """出力ファイルが見つからない場合、ConversionErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_audio:
                audio_path = tmp_audio.name
            with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_image:
                image_path = tmp_image.name
            
            try:
                mock_result = Mock()
                mock_result.returncode = 0
                mock_result.stderr = ""
                
                with patch("subprocess.run", return_value=mock_result):
                    with patch("os.path.exists") as mock_exists:
                        # 入力ファイルは存在、出力ファイルは存在しない
                        mock_exists.side_effect = [True, True, False]
                        
                        with pytest.raises(ConversionError, match="出力ファイルが見つかりません"):
                            converter.convert_audio_to_mp4_with_image(audio_path, image_path)
            finally:
                for path in [audio_path, image_path]:
                    if os.path.exists(path):
                        os.remove(path)

    def test_convert_audio_to_mp4_with_image_timeout(self):
        """ffmpeg実行がタイムアウトした場合、ConversionErrorが発生すること"""
        import subprocess
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_audio:
                audio_path = tmp_audio.name
            with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_image:
                image_path = tmp_image.name
            
            try:
                with patch("subprocess.run", side_effect=subprocess.TimeoutExpired("ffmpeg", 300)):
                    with patch("os.path.exists") as mock_exists:
                        mock_exists.side_effect = [True, True]
                        
                        with pytest.raises(ConversionError, match="タイムアウト"):
                            converter.convert_audio_to_mp4_with_image(audio_path, image_path)
            finally:
                for path in [audio_path, image_path]:
                    if os.path.exists(path):
                        os.remove(path)

    def test_convert_audio_to_mp4_with_image_unexpected_error(self):
        """予期しないエラーが発生した場合、ConversionErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_audio:
                audio_path = tmp_audio.name
            with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp_image:
                image_path = tmp_image.name
            
            try:
                with patch("subprocess.run", side_effect=RuntimeError("unexpected")):
                    with patch("os.path.exists") as mock_exists:
                        mock_exists.side_effect = [True, True]
                        
                        with pytest.raises(ConversionError, match="予期しないエラー"):
                            converter.convert_audio_to_mp4_with_image(audio_path, image_path)
            finally:
                for path in [audio_path, image_path]:
                    if os.path.exists(path):
                        os.remove(path)

    def test_get_media_info_file_not_found(self):
        """ファイルが存在しない場合、FileNotFoundErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            with pytest.raises(FileNotFoundError, match="ファイルが見つかりません"):
                converter.get_media_info("/nonexistent/file.mp4")

    def test_get_media_info_success(self):
        """メディア情報を正常に取得できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".mp4", delete=False) as tmp_file:
                file_path = tmp_file.name
                tmp_file.write(b"dummy content")
            
            try:
                mock_result = Mock()
                mock_result.returncode = 0
                mock_result.stderr = """
                Duration: 00:01:30.50, start: 0.000000
                Video: h264, yuv420p, 1920x1080
                Audio: aac, 48000 Hz
                """
                
                with patch("subprocess.run", return_value=mock_result):
                    with patch("os.path.exists", return_value=True):
                        info = converter.get_media_info(file_path)
                        
                        assert info["duration"] == "00:01:30.50"
                        assert info["video_codec"] == "h264"
                        assert info["audio_codec"] == "aac"
                        assert info["resolution"] == "1920x1080"
                        assert info["file_size"] > 0
            finally:
                if os.path.exists(file_path):
                    os.remove(file_path)

    def test_get_media_info_timeout(self):
        """情報取得がタイムアウトした場合、ConversionErrorが発生すること"""
        import subprocess
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".mp4", delete=False) as tmp_file:
                file_path = tmp_file.name
            
            try:
                with patch("subprocess.run", side_effect=subprocess.TimeoutExpired("ffmpeg", 30)):
                    with patch("os.path.exists", return_value=True):
                        with pytest.raises(ConversionError, match="タイムアウト"):
                            converter.get_media_info(file_path)
            finally:
                if os.path.exists(file_path):
                    os.remove(file_path)

    def test_get_media_info_unexpected_error(self):
        """予期しないエラーが発生した場合、ConversionErrorが発生すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(suffix=".mp4", delete=False) as tmp_file:
                file_path = tmp_file.name
            
            try:
                with patch("subprocess.run", side_effect=RuntimeError("unexpected")):
                    with patch("os.path.exists", return_value=True):
                        with pytest.raises(ConversionError, match="エラーが発生しました"):
                            converter.get_media_info(file_path)
            finally:
                if os.path.exists(file_path):
                    os.remove(file_path)

    def test_extract_duration(self):
        """Durationを正しく抽出できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            output = "Duration: 00:05:23.45, start: 0.000000"
            duration = converter._extract_duration(output)
            assert duration == "00:05:23.45"

    def test_extract_duration_not_found(self):
        """Durationが見つからない場合、Noneを返すこと"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            output = "No duration here"
            duration = converter._extract_duration(output)
            assert duration is None

    def test_extract_video_codec(self):
        """動画コーデックを正しく抽出できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            output = "Video: h264 (High), yuv420p"
            codec = converter._extract_video_codec(output)
            assert codec == "h264"

    def test_extract_video_codec_not_found(self):
        """動画コーデックが見つからない場合、Noneを返すこと"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            output = "No video codec here"
            codec = converter._extract_video_codec(output)
            assert codec is None

    def test_extract_audio_codec(self):
        """音声コーデックを正しく抽出できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            output = "Audio: aac (LC), 48000 Hz"
            codec = converter._extract_audio_codec(output)
            assert codec == "aac"

    def test_extract_audio_codec_not_found(self):
        """音声コーデックが見つからない場合、Noneを返すこと"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            output = "No audio codec here"
            codec = converter._extract_audio_codec(output)
            assert codec is None

    def test_extract_resolution(self):
        """解像度を正しく抽出できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            output = "Video: h264, yuv420p, 1920x1080, 30 fps"
            resolution = converter._extract_resolution(output)
            assert resolution == "1920x1080"

    def test_extract_resolution_not_found(self):
        """解像度が見つからない場合、Noneを返すこと"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            output = "No resolution here"
            resolution = converter._extract_resolution(output)
            assert resolution is None

    def test_cleanup_temp_files_success(self):
        """一時ファイルを正常にクリーンアップできること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            # 一時ファイルを作成
            with tempfile.NamedTemporaryFile(delete=False) as tmp_file:
                file_path = tmp_file.name
            
            assert os.path.exists(file_path)
            converter.cleanup_temp_files([file_path])
            assert not os.path.exists(file_path)

    def test_cleanup_temp_files_multiple(self):
        """複数の一時ファイルをクリーンアップできること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            # 複数の一時ファイルを作成
            file_paths = []
            for _ in range(3):
                with tempfile.NamedTemporaryFile(delete=False) as tmp_file:
                    file_paths.append(tmp_file.name)
            
            for path in file_paths:
                assert os.path.exists(path)
            
            converter.cleanup_temp_files(file_paths)
            
            for path in file_paths:
                assert not os.path.exists(path)

    def test_cleanup_temp_files_ignore_non_temp(self):
        """一時ディレクトリ外のファイルは削除しないこと"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            # 一時ディレクトリ外のファイルパスを指定
            non_temp_path = "/home/user/important.txt"
            
            with patch("os.path.exists", return_value=True):
                with patch("os.remove") as mock_remove:
                    converter.cleanup_temp_files([non_temp_path])
                    # 削除されないことを確認
                    mock_remove.assert_not_called()

    def test_cleanup_temp_files_ignore_errors(self):
        """ファイル削除エラーを無視すること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = MediaConverter()
            
            with tempfile.NamedTemporaryFile(delete=False) as tmp_file:
                file_path = tmp_file.name
            
            with patch("os.remove", side_effect=OSError("Permission denied")):
                # エラーが発生してもクラッシュしないこと
                converter.cleanup_temp_files([file_path])
            
            # クリーンアップ
            if os.path.exists(file_path):
                os.remove(file_path)


class TestFactoryFunctions:
    """ファクトリー関数のテスト"""

    def test_create_media_converter_default(self):
        """デフォルトパラメータでMediaConverterを作成できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = create_media_converter()
            assert isinstance(converter, MediaConverter)
            assert converter.ffmpeg_path == "ffmpeg"

    def test_create_media_converter_custom_path(self):
        """カスタムパスでMediaConverterを作成できること"""
        with patch.object(MediaConverter, "_check_ffmpeg_available"):
            converter = create_media_converter(ffmpeg_path="/usr/bin/ffmpeg")
            assert isinstance(converter, MediaConverter)
            assert converter.ffmpeg_path == "/usr/bin/ffmpeg"

    def test_create_media_converter_error(self):
        """ffmpegが利用できない場合、ConversionErrorが発生すること"""
        with patch("subprocess.run", side_effect=FileNotFoundError):
            with pytest.raises(ConversionError):
                create_media_converter()

    def test_convert_audio_for_x_success(self):
        """X投稿用の音声変換が成功すること"""
        with tempfile.NamedTemporaryFile(suffix=".m4a", delete=False) as tmp_file:
            input_path = tmp_file.name
        
        try:
            mock_result = Mock()
            mock_result.returncode = 0
            mock_result.stderr = ""
            
            with patch("subprocess.run", return_value=mock_result):
                with patch("os.path.exists") as mock_exists:
                    mock_exists.side_effect = [True, True]
                    
                    output_path = convert_audio_for_x(input_path)
                    assert output_path.endswith("_converted.mp4")
        finally:
            if os.path.exists(input_path):
                os.remove(input_path)

    def test_is_ffmpeg_available_true(self):
        """ffmpegが利用可能な場合、Trueを返すこと"""
        mock_result = Mock()
        mock_result.returncode = 0
        
        with patch("subprocess.run", return_value=mock_result):
            assert is_ffmpeg_available() is True

    def test_is_ffmpeg_available_false(self):
        """ffmpegが利用できない場合、Falseを返すこと"""
        with patch("subprocess.run", side_effect=FileNotFoundError):
            assert is_ffmpeg_available() is False
