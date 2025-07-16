import sys
from unittest.mock import patch
import pytest

@patch('src.article_manager.save_articles')
def test_main_does_not_save_articles_on_dry_run(mock_save):
    """--dry-run時はsave_articlesが呼ばれないことを確認"""
    import src.main
    test_args = ["main.py", "--dry-run"]
    with patch.object(sys, 'argv', test_args):
        src.main.main()
    mock_save.assert_not_called()
