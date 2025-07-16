import sys
from unittest.mock import patch, MagicMock

from src.main import main

@patch('src.main.load_config')
@patch('src.main.ArticleManager')
def test_main_dry_run_does_not_save_articles(mock_article_manager, mock_load_config):
    """ --dry-run時はArticleManager.save_articlesが呼ばれないことを確認 """
    # --- Arrange ---
    mock_load_config.return_value = {
        'blog': {'feed_url': 'http://test.com/feed'}
    }
    mock_am_instance = MagicMock()
    mock_article_manager.return_value = mock_am_instance
    mock_am_instance.get_new_articles.return_value = [
        {'title': 'New Article', 'link': 'http://new.com'}
    ]

    # --- Act ---
    with patch.object(sys, 'argv', ['src/main.py', '--dry-run']):
        main()

    # --- Assert ---
    mock_am_instance.save_articles.assert_not_called()