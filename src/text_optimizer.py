from typing import Any, Dict

from .url_shortener import URLShortener


class TextOptimizer:
    """
    SNS投稿用テキスト最適化クラス
    
    文字数制限に応じてURL短縮やタイトルトリミングを行います。
    """

    # SNS別デフォルト文字数制限
    DEFAULT_CHARACTER_LIMITS = {
        'x': 280,
        'bluesky': 300,
        'mastodon': 500,
        'misskey': 3000
    }

    def __init__(self, config: Dict[str, Any]):
        """
        TextOptimizerを初期化します
        
        Args:
            config (Dict[str, Any]): 設定辞書
        """
        self.config = config

        # URL短縮設定
        url_shortening_config = config.get('url_shortening', {})
        self.url_shortening_enabled = url_shortening_config.get('enabled', True)
        self.url_shortening_mode = url_shortening_config.get('mode', 'auto')

        # 文字数制限設定
        custom_limits = config.get('character_limits', {})
        self.character_limits = self.DEFAULT_CHARACTER_LIMITS.copy()
        self.character_limits.update(custom_limits)

        # URL短縮インスタンス
        if self.url_shortening_enabled:
            self.url_shortener = URLShortener()
        else:
            self.url_shortener = None

    def get_character_limit(self, sns_type: str) -> int:
        """
        指定されたSNSの文字数制限を取得します
        
        Args:
            sns_type (str): SNS種別
            
        Returns:
            int: 文字数制限
        """
        return self.character_limits.get(sns_type, 500)  # デフォルト500文字

    def optimize_text(self, title: str, link: str, sns_type: str, announcement: str = "", force_optimize: bool = False) -> str:
        """
        SNSの文字数制限に合わせてテキストを最適化します
        
        Args:
            title (str): 記事タイトル
            link (str): 記事URL
            sns_type (str): SNS種別
            announcement (str): アナウンス文（オプション）
            force_optimize (bool): 強制的に最適化を実行するかどうか
            
        Returns:
            str: 最適化されたテキスト
        """
        character_limit = self.get_character_limit(sns_type)

        # 基本形式: "{announcement} {title} {link}" または "{title} {link}"
        if announcement:
            original_text = f"{announcement} {title} {link}"
        else:
            original_text = f"{title} {link}"

        # URL短縮モードの判定
        should_shorten_url = self._should_shorten_url(original_text, character_limit, force_optimize)

        # 文字数制限内かつ強制最適化でない場合はそのまま返す
        if len(original_text) <= character_limit and not force_optimize:
            # alwaysモードの場合は短縮を適用
            if should_shorten_url:
                return self._apply_url_shortening(title, link, announcement, character_limit)
            return original_text

        # URL短縮を試行
        if should_shorten_url and self.url_shortener:
            shortened_link = self.url_shortener.shorten(link)

            if announcement:
                shortened_text = f"{announcement} {title} {shortened_link}"
            else:
                shortened_text = f"{title} {shortened_link}"

            # 短縮後に制限内に収まる場合
            if len(shortened_text) <= character_limit:
                return shortened_text

            # まだ長い場合はタイトルをトリミング
            return self._trim_title_with_link(title, shortened_link, character_limit, announcement)
        else:
            # URL短縮無効の場合、タイトルをトリミング
            return self._trim_title_with_link(title, link, character_limit, announcement)

    def _trim_title_with_link(self, title: str, link: str, character_limit: int, announcement: str = "") -> str:
        """
        タイトルをトリミングしてリンクと組み合わせます
        
        Args:
            title (str): 記事タイトル
            link (str): リンクURL（短縮済み可能性あり）
            character_limit (int): 文字数制限
            announcement (str): アナウンス文（オプション）
            
        Returns:
            str: トリミングされたテキスト
        """
        # "... " と link の分を確保
        ellipsis = "..."
        space = " "

        if announcement:
            # "{announcement} {title}... {link}" 形式
            announcement_part = f"{announcement} "
            reserved_length = len(announcement_part) + len(ellipsis) + len(space) + len(link)
        else:
            # "{title}... {link}" 形式
            announcement_part = ""
            reserved_length = len(ellipsis) + len(space) + len(link)

        # タイトルに使える文字数を計算
        available_title_length = character_limit - reserved_length

        # タイトルが使える文字数より短い場合はそのまま
        if len(title) <= available_title_length:
            if announcement:
                return f"{announcement} {title} {link}"
            else:
                return f"{title} {link}"

        # タイトルをトリミング
        if available_title_length > 0:
            trimmed_title = title[:available_title_length]
            if announcement:
                return f"{announcement} {trimmed_title}{ellipsis} {link}"
            else:
                return f"{trimmed_title}{ellipsis} {link}"
        else:
            # 極端に短い制限の場合
            if announcement:
                # アナウンス + リンクが制限を超える場合はリンクのみ
                if len(f"{announcement} {link}") <= character_limit:
                    return f"{announcement} {link}"
                else:
                    return link
            else:
                return link

    def _should_shorten_url(self, original_text: str, character_limit: int, force_optimize: bool) -> bool:
        """
        URL短縮を実行すべきかどうかを判定します
        
        Args:
            original_text (str): 元のテキスト
            character_limit (int): 文字数制限
            force_optimize (bool): 強制最適化フラグ
            
        Returns:
            bool: URL短縮を実行すべきかどうか
        """
        if not self.url_shortening_enabled:
            return False

        mode = self.url_shortening_mode

        if mode == "always":
            return True
        elif mode == "auto":
            return len(original_text) > character_limit or force_optimize
        elif mode == "never":
            return False
        else:
            # 未知のモードの場合はautoとして扱う
            return len(original_text) > character_limit or force_optimize

    def _apply_url_shortening(self, title: str, link: str, announcement: str, character_limit: int) -> str:
        """
        URL短縮を適用します
        
        Args:
            title (str): 記事タイトル
            link (str): 記事URL
            announcement (str): アナウンス文
            character_limit (int): 文字数制限
            
        Returns:
            str: URL短縮適用後のテキスト
        """
        if not self.url_shortener:
            # URL短縮が無効の場合は元のテキストを返す
            if announcement:
                return f"{announcement} {title} {link}"
            else:
                return f"{title} {link}"

        shortened_link = self.url_shortener.shorten(link)

        if announcement:
            shortened_text = f"{announcement} {title} {shortened_link}"
        else:
            shortened_text = f"{title} {shortened_link}"

        # 短縮後も制限を超える場合はタイトルをトリミング
        if len(shortened_text) > character_limit:
            return self._trim_title_with_link(title, shortened_link, character_limit, announcement)

        return shortened_text
