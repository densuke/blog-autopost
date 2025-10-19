"""
HTML処理共通ユーティリティのテスト
"""

from unittest.mock import Mock
from src.utils.html_utils import decode_html_content


class TestDecodeHtmlContent:
    """HTMLコンテンツデコード機能のテスト"""

    def test_decode_utf8_content(self):
        """UTF-8エンコードされたHTMLをデコードできることを確認"""
        html_content = "こんにちは世界".encode('utf-8')
        
        # モックレスポンスオブジェクト
        response = Mock()
        response.content = html_content
        response.headers = {'content-type': 'text/html; charset=utf-8'}
        
        result = decode_html_content(response)
        assert result == "こんにちは世界"

    def test_decode_sjis_content(self):
        """Shift_JISエンコードされたHTMLをデコードできることを確認"""
        html_content = "こんにちは世界".encode('shift_jis')
        
        response = Mock()
        response.content = html_content
        response.headers = {'content-type': 'text/html; charset=shift_jis'}
        
        result = decode_html_content(response)
        assert result == "こんにちは世界"

    def test_decode_with_meta_charset_tag(self):
        """meta charsetタグから文字コードを検出してデコードできることを確認"""
        html_content = '''<html>
<meta charset="shift_jis">
<body>こんにちは世界</body>
</html>'''.encode('shift_jis')
        
        response = Mock()
        response.content = html_content
        response.headers = {'content-type': 'text/html'}
        
        result = decode_html_content(response)
        assert "こんにちは世界" in result

    def test_decode_priority_meta_over_header(self):
        """meta charsetがContent-Typeヘッダーより優先されることを確認"""
        # メタタグではutf-8、ヘッダーではshift_jisを指定
        html_content = '''<html>
<meta charset="utf-8">
<body>テスト</body>
</html>'''.encode('utf-8')
        
        response = Mock()
        response.content = html_content
        response.headers = {'content-type': 'text/html; charset=shift_jis'}
        
        result = decode_html_content(response)
        assert "テスト" in result

    def test_decode_fallback_utf8_on_error(self):
        """デコード失敗時にUTF-8でフォールバックすることを確認"""
        # ランダムなバイナリデータ
        html_content = b'\x80\x81\x82\x83'
        
        response = Mock()
        response.content = html_content
        response.headers = {'content-type': 'text/html'}
        
        # 例外が発生しないことを確認
        result = decode_html_content(response)
        assert isinstance(result, str)

    def test_decode_html_with_article_data(self):
        """実際のHTMLコンテンツ構造でのデコード確認"""
        html_content = '''<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>テスト記事</title>
</head>
<body>
<h1>これはテスト記事です</h1>
<p>本文コンテンツ</p>
</body>
</html>'''.encode('utf-8')
        
        response = Mock()
        response.content = html_content
        response.headers = {'content-type': 'text/html; charset=utf-8'}
        
        result = decode_html_content(response)
        assert "テスト記事" in result
        assert "これはテスト記事です" in result

    def test_decode_empty_content(self):
        """空のコンテンツをデコードできることを確認"""
        response = Mock()
        response.content = b''
        response.headers = {'content-type': 'text/html'}
        
        result = decode_html_content(response)
        assert result == ""

    def test_decode_multiple_charset_formats(self):
        """複数の文字セット形式に対応できることを確認"""
        # メタタグの異なる形式
        test_cases = [
            '<meta charset="utf-8">',
            "<meta charset='utf-8'>",
            '<meta charset=utf-8>',
            '<meta http-equiv="Content-Type" content="text/html; charset=utf-8">',
        ]
        
        for charset_tag in test_cases:
            html_content = f'''<html>
{charset_tag}
<body>テスト</body>
</html>'''.encode('utf-8')
            
            response = Mock()
            response.content = html_content
            response.headers = {'content-type': 'text/html'}
            
            result = decode_html_content(response)
            assert "テスト" in result, f"Failed for charset_tag: {charset_tag}"

    def test_decode_no_charset_info(self):
        """文字セット情報がない場合のデコード確認"""
        html_content = b'<html><body>test</body></html>'
        
        response = Mock()
        response.content = html_content
        response.headers = {}
        
        result = decode_html_content(response)
        assert "test" in result
