import re

import requests


class URLShortener:
    """
    URL短縮サービスを提供するクラス
    
    is.gd APIを使用してURLを短縮します。
    APIキーは不要で、無料で利用できます。
    """

    def __init__(self):
        self.service = "is.gd"
        self.api_url = "https://is.gd/create.php"
        self.timeout = 10  # タイムアウト時間（秒）

    def shorten(self, url: str) -> str:
        """
        URLを短縮します
        
        Args:
            url (str): 短縮したい元のURL
            
        Returns:
            str: 短縮されたURL。失敗時は元のURLを返す
        """
        # 基本的なURL形式チェック
        if not self._is_valid_url(url):
            return url

        try:
            params = {
                "format": "simple",
                "url": url
            }

            response = requests.get(
                self.api_url,
                params=params,
                timeout=self.timeout
            )

            if response.status_code == 200:
                shortened_url = response.text.strip()

                # レスポンスがURLかどうかチェック
                if self._is_valid_url(shortened_url):
                    return shortened_url
                else:
                    # エラーメッセージが返された場合
                    print(f"URL短縮エラー: {shortened_url}")
                    return url
            else:
                print(f"URL短縮API呼び出し失敗: HTTP {response.status_code}")
                return url

        except requests.exceptions.Timeout:
            print(f"URL短縮APIタイムアウト: {self.timeout}秒")
            return url
        except requests.exceptions.RequestException as e:
            print(f"URL短縮API通信エラー: {e}")
            return url
        except Exception as e:
            print(f"URL短縮中の予期しないエラー: {e}")
            return url

    def _is_valid_url(self, url: str) -> bool:
        """
        URLの妥当性をチェックします
        
        Args:
            url (str): チェックするURL
            
        Returns:
            bool: 有効なURLの場合True
        """
        # 基本的なURL形式の正規表現
        url_pattern = re.compile(
            r'^https?://'  # http:// または https://
            r'(?:(?:[A-Z0-9](?:[A-Z0-9-]{0,61}[A-Z0-9])?\.)+[A-Z]{2,6}\.?|'  # ドメイン名
            r'localhost|'  # localhost
            r'\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})'  # IPアドレス
            r'(?::\d+)?'  # ポート番号（オプション）
            r'(?:/?|[/?]\S+)$', re.IGNORECASE)

        return bool(url_pattern.match(url))
