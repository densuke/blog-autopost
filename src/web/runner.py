import uvicorn

from ..config_manager import ConfigManager

if __name__ == "__main__":
    # 設定を読み込む
    config_manager = ConfigManager("config.yml")
    server_settings = config_manager.get_web_server_settings()
    host = server_settings.get("host", "127.0.0.1")
    port = server_settings.get("port", 8000)

    # Uvicornを起動
    uvicorn.run(
        "src.web.main_web:app",
        host=host,
        port=port,
        reload=True
    )
