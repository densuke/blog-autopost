#!/usr/bin/env python
# -*- coding: utf-8 -*-

from fastapi import FastAPI, Request
from fastapi.templating import Jinja2Templates

app = FastAPI()

templates = Jinja2Templates(directory="src/web/templates")

@app.get("/")
def read_root():
    return {"message": "Hello World"}

@app.get("/login")
def login(request: Request):
    return templates.TemplateResponse("login.html", {"request": request})
