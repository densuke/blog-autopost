import pytest
from PIL import Image
import io
import tempfile
import os
from src.image_resizer import ImageResizer, create_image_resizer


@pytest.fixture
def image_resizer():
    """ImageResizerインスタンスのフィクスチャ"""
    return ImageResizer(debug=False)


@pytest.fixture
def image_resizer_debug():
    """デバッグモード有効のImageResizerインスタンス"""
    return ImageResizer(debug=True)


@pytest.fixture
def sample_png_data():
    """サンプルPNG画像データを生成"""
    image = Image.new('RGB', (800, 600), color='red')
    buffer = io.BytesIO()
    image.save(buffer, format='PNG')
    return buffer.getvalue()


@pytest.fixture
def sample_jpeg_data():
    """サンプルJPEG画像データを生成"""
    image = Image.new('RGB', (1000, 800), color='blue')
    buffer = io.BytesIO()
    image.save(buffer, format='JPEG', quality=85)
    return buffer.getvalue()


@pytest.fixture
def sample_large_image_data():
    """大きなサンプル画像データを生成（リサイズが必要な大きさ）"""
    image = Image.new('RGB', (4096, 4096), color='green')
    buffer = io.BytesIO()
    image.save(buffer, format='JPEG', quality=85)
    return buffer.getvalue()


@pytest.fixture
def sample_rgba_image_data():
    """RGBAモードのサンプル画像データを生成"""
    image = Image.new('RGBA', (800, 600), color=(255, 0, 0, 128))
    buffer = io.BytesIO()
    image.save(buffer, format='PNG')
    return buffer.getvalue()


@pytest.fixture
def sample_gif_data():
    """サンプルGIF画像データを生成"""
    image = Image.new('RGB', (500, 400), color='yellow')
    buffer = io.BytesIO()
    image.save(buffer, format='GIF')
    return buffer.getvalue()


@pytest.fixture
def temp_image_file(sample_png_data):
    """テンポラリ画像ファイルのフィクスチャ"""
    with tempfile.NamedTemporaryFile(delete=False, suffix='.png') as temp_file:
        temp_file.write(sample_png_data)
        temp_path = temp_file.name
    
    yield temp_path
    
    # クリーンアップ
    if os.path.exists(temp_path):
        os.unlink(temp_path)


@pytest.fixture
def temp_large_image_file(sample_large_image_data):
    """テンポラリ大きなサイズ画像ファイルのフィクスチャ"""
    with tempfile.NamedTemporaryFile(delete=False, suffix='.jpg') as temp_file:
        temp_file.write(sample_large_image_data)
        temp_path = temp_file.name
    
    yield temp_path
    
    # クリーンアップ
    if os.path.exists(temp_path):
        os.unlink(temp_path)


# ===== 基本的なリサイズ成功ケース =====

def test_resize_image_data_png_success(image_resizer, sample_png_data):
    """resize_image_data が PNG 画像を正常にリサイズできることをテスト"""
    result = image_resizer.resize_image_data(sample_png_data, sns_type='bluesky')
    
    assert isinstance(result, bytes)
    assert len(result) > 0
    # Blueskyの制限内（1MB以下）であることを確認
    assert len(result) <= image_resizer.SNS_LIMITS['bluesky']['max_file_size']


def test_resize_image_data_jpeg_success(image_resizer, sample_jpeg_data):
    """resize_image_data が JPEG 画像を正常にリサイズできることをテスト"""
    result = image_resizer.resize_image_data(sample_jpeg_data, sns_type='x')
    
    assert isinstance(result, bytes)
    assert len(result) > 0
    # Xの制限内（5MB以下）であることを確認
    assert len(result) <= image_resizer.SNS_LIMITS['x']['max_file_size']


def test_resize_image_data_large_to_bluesky(image_resizer, sample_large_image_data):
    """resize_image_data が大きな画像をBluesky制限に縮小できることをテスト"""
    # 元の大きなサイズ
    original_size = len(sample_large_image_data)
    
    result = image_resizer.resize_image_data(sample_large_image_data, sns_type='bluesky')
    
    assert isinstance(result, bytes)
    # リサイズ後のサイズが制限内であることを確認
    assert len(result) <= image_resizer.SNS_LIMITS['bluesky']['max_file_size']
    # リサイズされているはず（元のサイズより小さくなっている可能性が高い）
    assert len(result) <= original_size


def test_resize_image_data_aspect_ratio_preserved(image_resizer):
    """resize_image_data がアスペクト比を保持していることをテスト"""
    # 16:9のアスペクト比の画像を作成
    original_image = Image.new('RGB', (1600, 900), color='cyan')
    buffer = io.BytesIO()
    original_image.save(buffer, format='JPEG', quality=85)
    image_data = buffer.getvalue()
    
    result = image_resizer.resize_image_data(image_data, sns_type='bluesky')
    
    # 結果の画像を開いてアスペクト比を確認
    result_image = Image.open(io.BytesIO(result))
    original_aspect = original_image.width / original_image.height
    result_aspect = result_image.width / result_image.height
    
    # アスペクト比がほぼ同じであることを確認（±0.01の誤差を許容）
    assert abs(original_aspect - result_aspect) < 0.01


def test_resize_image_data_rgba_conversion(image_resizer, sample_rgba_image_data):
    """resize_image_data が RGBA 画像を正しく処理できることをテスト"""
    result = image_resizer.resize_image_data(sample_rgba_image_data, sns_type='bluesky')
    
    assert isinstance(result, bytes)
    assert len(result) > 0
    # リサイズ後のデータを開いて有効な画像であることを確認
    result_image = Image.open(io.BytesIO(result))
    # RGBAモードはRGBに変換される、またはそのまま保持される場合がある
    assert result_image.mode in ['RGB', 'RGBA', 'P']


def test_resize_image_data_gif_conversion(image_resizer, sample_gif_data):
    """resize_image_data が GIF 画像を正しく処理できることをテスト"""
    result = image_resizer.resize_image_data(sample_gif_data, sns_type='bluesky')
    
    assert isinstance(result, bytes)
    assert len(result) > 0
    # リサイズ後のサイズが制限内であることを確認
    assert len(result) <= image_resizer.SNS_LIMITS['bluesky']['max_file_size']


# ===== エラーハンドリング =====

def test_resize_image_data_invalid_format(image_resizer):
    """resize_image_data が無効なデータを受け取った場合、元データを返すことをテスト"""
    invalid_data = b"This is not an image"
    
    result = image_resizer.resize_image_data(invalid_data, sns_type='bluesky')
    
    # エラー時は元データを返す仕様
    assert result == invalid_data


def test_resize_image_data_empty_data(image_resizer):
    """resize_image_data が空のデータを受け取った場合、元データを返すことをテスト"""
    empty_data = b""
    
    result = image_resizer.resize_image_data(empty_data, sns_type='bluesky')
    
    assert result == empty_data


def test_resize_image_data_unknown_sns_type(image_resizer, sample_png_data):
    """resize_image_data が未知のSNS種類を受け取った場合、デフォルト制限を適用することをテスト"""
    result = image_resizer.resize_image_data(sample_png_data, sns_type='unknown_sns')
    
    assert isinstance(result, bytes)
    # デフォルトはblueskeyの制限が適用される
    assert len(result) <= image_resizer.SNS_LIMITS['bluesky']['max_file_size']


# ===== ファイルベースのリサイズ =====

def test_resize_image_file_success(image_resizer, temp_image_file):
    """resize_image_file が画像ファイルを正常にリサイズできることをテスト"""
    result_path = image_resizer.resize_image_file(temp_image_file, sns_type='bluesky')
    
    try:
        assert isinstance(result_path, str)
        assert os.path.exists(result_path)
        
        # リサイズ後のファイルサイズが制限内であることを確認
        file_size = os.path.getsize(result_path)
        assert file_size <= image_resizer.SNS_LIMITS['bluesky']['max_file_size']
    finally:
        # クリーンアップ
        if result_path != temp_image_file and os.path.exists(result_path):
            os.unlink(result_path)


def test_resize_image_file_large_image(image_resizer, temp_large_image_file):
    """resize_image_file が大きな画像ファイルをリサイズできることをテスト"""
    original_size = os.path.getsize(temp_large_image_file)
    
    result_path = image_resizer.resize_image_file(temp_large_image_file, sns_type='bluesky')
    
    try:
        assert isinstance(result_path, str)
        assert os.path.exists(result_path)
        
        result_size = os.path.getsize(result_path)
        # リサイズ後のサイズが制限内であることを確認
        assert result_size <= image_resizer.SNS_LIMITS['bluesky']['max_file_size']
    finally:
        if result_path != temp_large_image_file and os.path.exists(result_path):
            os.unlink(result_path)


def test_resize_image_file_nonexistent(image_resizer):
    """resize_image_file が存在しないファイルを渡された場合、元ファイルパスを返すことをテスト"""
    nonexistent_path = "/nonexistent/path/to/image.jpg"
    
    result_path = image_resizer.resize_image_file(nonexistent_path, sns_type='bluesky')
    
    # エラー時は元ファイルパスを返す仕様
    assert result_path == nonexistent_path


def test_resize_image_file_creates_temp_file(image_resizer, temp_image_file):
    """resize_image_file が一時ファイルを作成し、異なるパスを返すことをテスト"""
    result_path = image_resizer.resize_image_file(temp_image_file, sns_type='bluesky')
    
    try:
        # 異なるファイルパスが返されることを確認
        assert result_path != temp_image_file
        # 一時ディレクトリ内にあることを確認
        assert result_path.startswith(tempfile.gettempdir())
    finally:
        if os.path.exists(result_path):
            os.unlink(result_path)


# ===== SNS制限情報の取得 =====

def test_get_sns_limits_bluesky(image_resizer):
    """get_sns_limits が Bluesky の制限情報を正しく返すことをテスト"""
    limits = image_resizer.get_sns_limits('bluesky')
    
    assert isinstance(limits, dict)
    assert limits['max_file_size'] == 1024 * 1024  # 1MB
    assert limits['max_width'] == 2000
    assert limits['max_height'] == 2000
    assert limits['quality'] == 85


def test_get_sns_limits_x(image_resizer):
    """get_sns_limits が X の制限情報を正しく返すことをテスト"""
    limits = image_resizer.get_sns_limits('x')
    
    assert isinstance(limits, dict)
    assert limits['max_file_size'] == 5 * 1024 * 1024  # 5MB
    assert limits['max_width'] == 4096
    assert limits['max_height'] == 4096
    assert limits['quality'] == 85


def test_get_sns_limits_mastodon(image_resizer):
    """get_sns_limits が Mastodon の制限情報を正しく返すことをテスト"""
    limits = image_resizer.get_sns_limits('mastodon')
    
    assert isinstance(limits, dict)
    assert limits['max_file_size'] == 10 * 1024 * 1024  # 10MB
    assert limits['max_width'] == 1920
    assert limits['max_height'] == 1920
    assert limits['quality'] == 85


def test_get_sns_limits_misskey(image_resizer):
    """get_sns_limits が Misskey の制限情報を正しく返すことをテスト"""
    limits = image_resizer.get_sns_limits('misskey')
    
    assert isinstance(limits, dict)
    assert limits['max_file_size'] == 10 * 1024 * 1024  # 10MB
    assert limits['max_width'] == 2048
    assert limits['max_height'] == 2048
    assert limits['quality'] == 85


def test_get_sns_limits_unknown(image_resizer):
    """get_sns_limits が未知のSNS種類を受け取った場合、デフォルト制限を返すことをテスト"""
    limits = image_resizer.get_sns_limits('unknown_sns')
    
    # デフォルトはblueskeyの制限が返される
    assert limits == image_resizer.SNS_LIMITS['bluesky']


# ===== 一時ファイルのクリーンアップ =====

def test_cleanup_temp_file_success(image_resizer):
    """cleanup_temp_file が一時ファイルを正常に削除できることをテスト"""
    # 一時ファイルを作成
    with tempfile.NamedTemporaryFile(delete=False, suffix='.jpg') as temp_file:
        temp_file.write(b"test data")
        temp_path = temp_file.name
    
    assert os.path.exists(temp_path)
    
    # クリーンアップ実行
    ImageResizer.cleanup_temp_file(temp_path)
    
    # ファイルが削除されたことを確認
    assert not os.path.exists(temp_path)


def test_cleanup_temp_file_nonexistent(image_resizer):
    """cleanup_temp_file が存在しないファイルを受け取った場合、エラーなく完了することをテスト"""
    nonexistent_path = "/tmp/nonexistent_file_12345.jpg"
    
    # エラーが発生しないことを確認
    ImageResizer.cleanup_temp_file(nonexistent_path)


def test_cleanup_temp_file_outside_temp_dir(image_resizer):
    """cleanup_temp_file が一時ディレクトリ外のファイルを受け取った場合、削除しないことをテスト"""
    # 一時ディレクトリ外のファイルパス
    non_temp_path = "/home/user/image.jpg"
    
    # cleanup_temp_file は一時ディレクトリ内のファイルのみ削除する仕様
    # したがってエラーなく完了すること
    ImageResizer.cleanup_temp_file(non_temp_path)


def test_cleanup_temp_file_empty_path(image_resizer):
    """cleanup_temp_file が空のパスを受け取った場合、エラーなく完了することをテスト"""
    ImageResizer.cleanup_temp_file("")


def test_cleanup_temp_file_none(image_resizer):
    """cleanup_temp_file が None を受け取った場合、エラーなく完了することをテスト"""
    ImageResizer.cleanup_temp_file(None)


# ===== ファクトリ関数テスト =====

def test_create_image_resizer_default():
    """create_image_resizer がデフォルト設定でインスタンスを作成できることをテスト"""
    resizer = create_image_resizer()
    
    assert isinstance(resizer, ImageResizer)
    assert resizer.debug is False


def test_create_image_resizer_debug_mode():
    """create_image_resizer がデバッグモードを有効にしてインスタンスを作成できることをテスト"""
    resizer = create_image_resizer(debug=True)
    
    assert isinstance(resizer, ImageResizer)
    assert resizer.debug is True


# ===== 統合テスト =====

def test_resize_workflow_png_to_bluesky(image_resizer, sample_png_data):
    """PNG画像をBluesky用にリサイズするワークフローをテスト"""
    # データのリサイズ
    resized_data = image_resizer.resize_image_data(sample_png_data, sns_type='bluesky')
    
    assert isinstance(resized_data, bytes)
    assert len(resized_data) <= image_resizer.SNS_LIMITS['bluesky']['max_file_size']
    
    # リサイズ後のデータが有効な画像であることを確認
    result_image = Image.open(io.BytesIO(resized_data))
    assert result_image.width <= image_resizer.SNS_LIMITS['bluesky']['max_width']
    assert result_image.height <= image_resizer.SNS_LIMITS['bluesky']['max_height']


def test_resize_workflow_large_image_to_x(image_resizer, sample_large_image_data):
    """大きな画像をX用にリサイズするワークフローをテスト"""
    resized_data = image_resizer.resize_image_data(sample_large_image_data, sns_type='x')
    
    assert isinstance(resized_data, bytes)
    assert len(resized_data) <= image_resizer.SNS_LIMITS['x']['max_file_size']
    
    # リサイズ後の画像が有効であることを確認
    result_image = Image.open(io.BytesIO(resized_data))
    assert result_image.format in ['JPEG', None]  # JPEGまたはバイナリ形式


def test_multiple_sns_resize_consistency(image_resizer, sample_png_data):
    """同じ画像を複数のSNS向けにリサイズした場合、各制限が守られることをテスト"""
    sns_types = ['bluesky', 'x', 'mastodon', 'misskey']
    
    for sns_type in sns_types:
        result = image_resizer.resize_image_data(sample_png_data, sns_type=sns_type)
        limits = image_resizer.SNS_LIMITS[sns_type]
        
        assert len(result) <= limits['max_file_size']
        
        result_image = Image.open(io.BytesIO(result))
        assert result_image.width <= limits['max_width']
        assert result_image.height <= limits['max_height']
