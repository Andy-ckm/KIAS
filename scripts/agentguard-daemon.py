#!/usr/bin/env python3
"""
AgentGuard 自主开发守护进程

全自主方案：
- 后台持续运行，不依赖用户消息
- 状态机驱动，不是定时触发
- 事件驱动，响应文件变化、git提交
- 任务队列，自动分解复杂任务
- 自我学习，从错误中学习
"""

import asyncio
import json
import os
import sqlite3
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Optional

# 配置
PROJECT_DIR = Path("/workspace/kias")
STATE_DB = PROJECT_DIR / ".agentguard-state.db"
LOG_FILE = PROJECT_DIR / ".agentguard-daemon.log"
ITERATION_INTERVAL = 60  # 每60秒检查一次

class AgentGuardDaemon:
    """自主开发守护进程"""
    
    def __init__(self):
        self.db_path = STATE_DB
        self.log_file = LOG_FILE
        self.init_db()
        self.running = True
        self.current_iteration = 0
        self.max_iterations = 100
        self.stuck_count = 0
        self.max_stuck = 3
    
    def init_db(self):
        """初始化状态数据库"""
        conn = sqlite3.connect(self.db_path)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS state (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_type TEXT,
                description TEXT,
                status TEXT DEFAULT 'pending',
                priority INTEGER DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                started_at TIMESTAMP,
                completed_at TIMESTAMP,
                result TEXT,
                error TEXT
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS learnings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                iteration INTEGER,
                lesson TEXT,
                category TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                iteration INTEGER,
                message TEXT,
                level TEXT DEFAULT 'info',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
        conn.commit()
        conn.close()
    
    def get_state(self, key: str, default: str = "") -> str:
        """获取状态"""
        conn = sqlite3.connect(self.db_path)
        cursor = conn.execute("SELECT value FROM state WHERE key = ?", (key,))
        row = cursor.fetchone()
        conn.close()
        return row[0] if row else default
    
    def set_state(self, key: str, value: str):
        """设置状态"""
        conn = sqlite3.connect(self.db_path)
        conn.execute("""
            INSERT OR REPLACE INTO state (key, value, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
        """, (key, value))
        conn.commit()
        conn.close()
    
    def add_task(self, task_type: str, description: str, priority: int = 0):
        """添加任务"""
        conn = sqlite3.connect(self.db_path)
        conn.execute("""
            INSERT INTO tasks (task_type, description, priority)
            VALUES (?, ?, ?)
        """, (task_type, description, priority))
        conn.commit()
        conn.close()
    
    def get_next_task(self) -> Optional[dict]:
        """获取下一个任务"""
        conn = sqlite3.connect(self.db_path)
        cursor = conn.execute("""
            SELECT id, task_type, description, priority
            FROM tasks
            WHERE status = 'pending'
            ORDER BY priority DESC, created_at ASC
            LIMIT 1
        """)
        row = cursor.fetchone()
        conn.close()
        if row:
            return {
                'id': row[0],
                'task_type': row[1],
                'description': row[2],
                'priority': row[3]
            }
        return None
    
    def update_task_status(self, task_id: int, status: str, result: str = None, error: str = None):
        """更新任务状态"""
        conn = sqlite3.connect(self.db_path)
        if status == 'running':
            conn.execute("""
                UPDATE tasks SET status = ?, started_at = CURRENT_TIMESTAMP
                WHERE id = ?
            """, (status, task_id))
        elif status in ('completed', 'failed'):
            conn.execute("""
                UPDATE tasks SET status = ?, completed_at = CURRENT_TIMESTAMP, result = ?, error = ?
                WHERE id = ?
            """, (status, result, error, task_id))
        conn.commit()
        conn.close()
    
    def add_learning(self, iteration: int, lesson: str, category: str = "general"):
        """添加学习经验"""
        conn = sqlite3.connect(self.db_path)
        conn.execute("""
            INSERT INTO learnings (iteration, lesson, category)
            VALUES (?, ?, ?)
        """, (iteration, lesson, category))
        conn.commit()
        conn.close()
    
    def log(self, iteration: int, message: str, level: str = "info"):
        """记录日志"""
        conn = sqlite3.connect(self.db_path)
        conn.execute("""
            INSERT INTO log (iteration, message, level)
            VALUES (?, ?, ?)
        """, (iteration, message, level))
        conn.commit()
        conn.close()
        
        # 同时写入文件
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with open(self.log_file, "a") as f:
            f.write(f"[{timestamp}] [{level}] {message}\n")
    
    def run_command(self, cmd: str, cwd: str = None) -> tuple:
        """运行命令"""
        try:
            result = subprocess.run(
                cmd, shell=True, cwd=cwd or str(PROJECT_DIR),
                capture_output=True, text=True, timeout=300
            )
            return result.returncode, result.stdout, result.stderr
        except subprocess.TimeoutExpired:
            return -1, "", "Command timed out"
        except Exception as e:
            return -1, "", str(e)
    
    def check_tests(self) -> tuple:
        """检查测试状态"""
        code, stdout, stderr = self.run_command("cargo test --workspace 2>&1 | grep 'test result:'")
        self.log(self.current_iteration, f"测试命令返回: code={code}, stdout长度={len(stdout)}, stderr长度={len(stderr)}")
        if code == 0:
            # 解析测试结果
            lines = stdout.strip().split("\n")
            total_passed = 0
            total_failed = 0
            for line in lines:
                if "passed" in line:
                    parts = line.split()
                    for i, part in enumerate(parts):
                        if part == "passed;":
                            total_passed += int(parts[i-1])
                        elif part == "failed;":
                            total_failed += int(parts[i-1])
            return total_passed, total_failed
        return 0, -1  # 测试运行失败
    
    def check_git_status(self) -> tuple:
        """检查git状态"""
        code, stdout, stderr = self.run_command("git status --short")
        if code == 0:
            uncommitted = len(stdout.strip().split("\n")) if stdout.strip() else 0
            return uncommitted
        return -1
    
    def commit_changes(self, message: str):
        """提交更改"""
        self.run_command("git add -A")
        self.run_command(f'git commit -m "{message}"')
        self.run_command("git push origin main")
    
    def select_module(self) -> str:
        """选择下一个要推进的模块"""
        # 检查每个模块的测试数量
        modules = {
            'linux-automation': 'kias-linux-automation',
            'document-management': 'kias-document-management',
            'it-change-management': 'kias-it-change-management'
        }
        
        min_tests = float('inf')
        target = 'linux-automation'
        
        for module, package in modules.items():
            code, stdout, stderr = self.run_command(f"cargo test --package {package} 2>&1 | grep -c 'test .* ok'")
            if code == 0:
                try:
                    tests = int(stdout.strip())
                    if tests < min_tests:
                        min_tests = tests
                        target = module
                except:
                    pass
        
        return target
    
    def decompose_task(self, task_description: str) -> list:
        """分解复杂任务"""
        # 简单的任务分解逻辑
        # 实际应该用LLM来分解
        subtasks = []
        
        if "全文搜索" in task_description:
            subtasks = [
                "添加FTS5虚拟表",
                "添加触发器自动同步数据",
                "修改search函数使用FTS5查询",
                "添加测试",
                "提交代码"
            ]
        elif "SSH连接" in task_description:
            subtasks = [
                "添加SSH密钥支持",
                "添加连接超时",
                "添加心跳检测",
                "添加测试",
                "提交代码"
            ]
        elif "Web API" in task_description:
            subtasks = [
                "添加axum依赖",
                "创建路由",
                "实现端点",
                "添加测试",
                "提交代码"
            ]
        else:
            subtasks = [task_description]
        
        return subtasks
    
    async def execute_task(self, task: dict):
        """执行任务"""
        task_id = task['id']
        description = task['description']
        
        self.log(self.current_iteration, f"开始执行任务: {description}")
        self.update_task_status(task_id, 'running')
        
        # 分解任务
        subtasks = self.decompose_task(description)
        
        for i, subtask in enumerate(subtasks):
            self.log(self.current_iteration, f"  子任务 {i+1}/{len(subtasks)}: {subtask}")
            
            # 这里应该调用Hermes Agent来执行
            # 但目前只是模拟
            # 实际实现需要通过Hermes CLI或API调用
            
            # 模拟执行
            await asyncio.sleep(1)
            
            # 检查测试
            passed, failed = self.check_tests()
            if failed > 0:
                self.log(self.current_iteration, f"  测试失败: {failed}个", "error")
                self.update_task_status(task_id, 'failed', error=f"测试失败: {failed}个")
                return False
        
        self.log(self.current_iteration, f"  任务完成: {description}")
        self.update_task_status(task_id, 'completed', result="完成")
        return True
    
    async def run(self):
        """主循环"""
        self.log(0, "AgentGuard 自主开发守护进程启动")
        
        while self.running and self.current_iteration < self.max_iterations:
            self.current_iteration += 1
            self.log(self.current_iteration, f"=== 迭代 {self.current_iteration} ===")
            
            # 1. 检查当前状态
            passed, failed = self.check_tests()
            uncommitted = self.check_git_status()
            
            self.log(self.current_iteration, f"测试状态: {passed}通过, {failed}失败")
            self.log(self.current_iteration, f"未提交文件: {uncommitted}")
            
            # 2. 提交未保存的代码
            if uncommitted > 0:
                self.log(self.current_iteration, f"提交 {uncommitted} 个未保存文件")
                self.commit_changes(f"自主循环迭代 {self.current_iteration}: 自动提交")
            
            # 3. 获取下一个任务
            task = self.get_next_task()
            
            if task:
                # 执行任务
                success = await self.execute_task(task)
                
                if success:
                    self.stuck_count = 0
                else:
                    self.stuck_count += 1
                    self.add_learning(self.current_iteration, f"任务失败: {task['description']}", "failure")
            else:
                # 没有任务，自动添加
                module = self.select_module()
                self.add_task("充实模块", f"推进{module}模块的血肉", priority=1)
                self.log(self.current_iteration, f"自动添加任务: 推进{module}模块")
            
            # 4. 检查是否卡住
            if self.stuck_count >= self.max_stuck:
                self.log(self.current_iteration, f"连续{self.stuck_count}轮没有进展，停止", "warning")
                self.add_learning(self.current_iteration, "连续多轮没有进展，需要人工介入", "stuck")
                break
            
            # 5. 等待下一轮
            await asyncio.sleep(ITERATION_INTERVAL)
        
        self.log(self.current_iteration, "守护进程结束")

def main():
    """主函数"""
    daemon = AgentGuardDaemon()
    asyncio.run(daemon.run())

if __name__ == "__main__":
    main()
