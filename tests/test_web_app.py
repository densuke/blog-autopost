#!/usr/bin/env python
# -*- coding: utf-8 -*-

import pytest
from fastapi import FastAPI

def test_fastapi_instance():
    """src.web.main_webにFastAPIインスタンスが存在することを確認する"""
    try:
        from src.web.main_web import app
        assert isinstance(app, FastAPI)
    except ImportError:
        pytest.fail("src.web.main_webまたはappインスタンスが見つかりません")
