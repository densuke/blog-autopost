"""
HTML処理ユーティリティ

HTMLコンテンツの文字コード検出やデコード処理を提供します。
main.py と bluesky.py で共通化されていた処理を統合。
"""

import re


def decode_html_content(response) -> str:
    """
    HTMLコンテンツを適切な文字コードでデコードします。
    
    Content-Typeヘッダーと meta charset タグから文字コードを検出し、
    複数の文字コードを試行してデコードを試みます。
    
    Args:
        response: requestsのレスポンスオブジェクト
        
    Returns:
        str: デコードされたHTMLコンテンツ
    """
    # Content-Typeヘッダーからcharsetを取得
    content_type = response.headers.get('content-type', '').lower()

    # HTMLのmeta charsetタグから文字コードを検出
    html_bytes = response.content
    html_preview = html_bytes[:2048].decode('utf-8', errors='ignore').lower()

    # meta charset検出のパターン
    charset_patterns = [
        r'<meta[^>]+charset=["\']?([^"\'>\\s]+)',
        r'<meta[^>]+content=["\'][^\'"]*charset=([^"\'>\\s]+)',
    ]

    detected_charset = None
    for pattern in charset_patterns:
        match = re.search(pattern, html_preview)
        if match:
            detected_charset = match.group(1).strip()
            break

    # 文字コード優先順位: meta charset > Content-Type > 自動検出
    encodings_to_try = []

    if detected_charset:
        encodings_to_try.append(detected_charset)

    if 'charset=' in content_type:
        charset_from_header = content_type.split('charset=')[1].split(';')[0].strip()
        if charset_from_header not in encodings_to_try:
            encodings_to_try.append(charset_from_header)

    # 日本語サイトでよく使われる文字コードを追加
    common_encodings = ['utf-8', 'shift_jis', 'euc-jp', 'iso-2022-jp', 'cp932']
    for encoding in common_encodings:
        if encoding not in encodings_to_try:
            encodings_to_try.append(encoding)

    # 各文字コードを順番に試行
    for encoding in encodings_to_try:
        try:
            return html_bytes.decode(encoding)
        except (UnicodeDecodeError, LookupError):
            continue

    # すべて失敗した場合はUTF-8でエラーを無視してデコード
    return html_bytes.decode('utf-8', errors='ignore')
