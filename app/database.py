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


def migrate_db():
    """
    数据库迁移：检查并添加缺失的字段。
    支持 SQLite 的 ALTER TABLE ADD COLUMN。
    """
    with Session(engine) as session:
        # 检查 billing_interval 字段是否存在
        result = session.exec(
            "SELECT name FROM pragma_table_info('subscription') WHERE name='billing_interval'"
        )
        if not result.first():
            # 添加 billing_interval 字段
            session.exec(
                "ALTER TABLE subscription ADD COLUMN billing_interval INTEGER DEFAULT 1 NOT NULL"
            )
            session.commit()


def init_db():
    """
    初始化数据库，创建所有表结构并执行迁移。
    在应用启动时调用一次即可。
    """
    SQLModel.metadata.create_all(engine)
    migrate_db()


def get_session():
    """
    获取数据库会话的依赖注入函数。
    用于 FastAPI 的 Depends 注入。
    """
    with Session(engine) as session:
        yield session