"""
数据库配置模块
负责初始化 SQLite 数据库连接和创建表结构
"""
import os
from sqlmodel import SQLModel, create_engine, Session

# 数据库文件路径
DATABASE_URL = os.getenv("DATABASE_URL", "sqlite:///./sub_manager.db")

# 创建数据库引擎
engine = create_engine(
    DATABASE_URL,
    echo=False,
    connect_args={"check_same_thread": False}  # SQLite 特定配置
)


def init_db():
    """
    初始化数据库，创建所有表结构。
    在应用启动时调用一次即可。
    """
    SQLModel.metadata.create_all(engine)


def get_session():
    """
    获取数据库会话的依赖注入函数。
    用于 FastAPI 的 Depends 注入。
    """
    with Session(engine) as session:
        yield session