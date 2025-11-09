"""config.yml.templateのサンプル設定を検証するテスト"""

from pathlib import Path
from typing import Any

import yaml

TEMPLATE_PATH = Path(__file__).resolve().parent.parent / "config.yml.template"


def _load_template() -> dict[str, Any]:
    """テンプレートとなるYAMLを読み込み辞書として返す"""
    with TEMPLATE_PATH.open("r", encoding="utf-8") as file:
        data = yaml.safe_load(file)
    assert isinstance(data, dict)
    return data


def test_template_has_web_auth_credentials() -> None:
    """web_authセクションに必須キーが含まれていることを確認する"""
    template = _load_template()
    web_auth = template.get("web_auth")
    assert isinstance(web_auth, dict)
    assert isinstance(web_auth.get("username"), str) and web_auth["username"].strip()
    assert isinstance(web_auth.get("password"), str) and web_auth["password"].strip()
    assert (
        isinstance(web_auth.get("secret_key"), str) and web_auth["secret_key"].strip()
    )


def test_template_has_blog_feeds() -> None:
    """blogセクションにフィード設定が含まれることを確認する"""
    template = _load_template()
    blog = template.get("blog")
    assert isinstance(blog, list) and blog
    first_feed = blog[0]
    assert isinstance(first_feed, dict)
    assert isinstance(first_feed.get("name"), str) and first_feed["name"].strip()
    assert (
        isinstance(first_feed.get("feed_url"), str) and first_feed["feed_url"].strip()
    )
    image_settings = first_feed.get("image_settings")
    assert isinstance(image_settings, dict)


def test_template_includes_timing_configuration() -> None:
    """投稿タイミング設定のサンプルが揃っていることを確認する"""
    template = _load_template()
    default_timings = template.get("default_allowed_timings")
    assert isinstance(default_timings, list) and default_timings
    assert isinstance(default_timings[0], list) and len(default_timings[0]) == 2
    tolerance = template.get("allowed_timings_tolerance_minutes")
    assert isinstance(tolerance, int)
    allowed_timings = template.get("allowed_timings")
    assert isinstance(allowed_timings, dict) and allowed_timings
    sample_key, sample_value = next(iter(allowed_timings.items()))
    assert isinstance(sample_key, str) and sample_key.strip()
    assert isinstance(sample_value, list) and sample_value
    assert isinstance(sample_value[0], list) and len(sample_value[0]) == 2


def test_template_has_sns_entries() -> None:
    """snsセクションに複数アカウントの雛形が含まれることを確認する"""
    template = _load_template()
    sns_entries = template.get("sns")
    assert isinstance(sns_entries, list) and sns_entries
    first_entry = sns_entries[0]
    assert isinstance(first_entry, dict)
    assert isinstance(first_entry.get("type"), str) and first_entry["type"].strip()
    assert isinstance(first_entry.get("name"), str) and first_entry["name"].strip()
