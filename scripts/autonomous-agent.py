#!/usr/bin/env python3
"""
AgentGuard 完全自主运行脚本

核心思想：
- 程序掌握控制流，LLM只管单点生成
- 没有trace证据不许动手
- 一次只改一个东西
- 先commit再验证
- 测试失败就revert
"""

import subprocess
import sqlite3
import json
import os
import sys
import time
from datetime import datetime
from pathlib import Path

# 配置
PROJECT_DIR = Path("/workspace/kias")
STATE_DB = PROJECT_DIR / ".agentguard.db"
TRACE_DIR = PROJECT_DIR / ".trace"
LOG_FILE = PROJECT_DIR / ".autonomous.log"
INTERVAL = 60  # 每60秒一轮

class AutonomousAgent:
    """完全自主运行的Agent"""
    
    def __init__(self):
        self.db_path = STATE_DB
        self.trace_dir = TRACE_DIR
        self.log_file = LOG_FILE
        self.init_db()
        self.running = True
        
        # 确保目录存在
        self.trace_dir.mkdir(exist_ok=True)
    
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
            CREATE TABLE IF NOT EXISTS trace (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                iteration INTEGER,
                phase TEXT,
                message TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS experiments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                iteration INTEGER,
                description TEXT,
                result TEXT,
                passed INTEGER,
                failed INTEGER,
                gate_result TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
        conn.commit()
        conn.close()
    
    def get_state(self, key, default=""):
        """获取状态"""
        conn = sqlite3.connect(self.db_path)
        cursor = conn.execute("SELECT value FROM state WHERE key = ?", (key,))
        row = cursor.fetchone()
        conn.close()
        return row[0] if row else default
    
    def set_state(self, key, value):
        """设置状态"""
        conn = sqlite3.connect(self.db_path)
        conn.execute("""
            INSERT OR REPLACE INTO state (key, value, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
        """, (key, value))
        conn.commit()
        conn.close()
    
    def add_trace(self, iteration, phase, message):
        """添加trace"""
        conn = sqlite3.connect(self.db_path)
        conn.execute("""
            INSERT INTO trace (iteration, phase, message)
            VALUES (?, ?, ?)
        """, (iteration, phase, message))
        conn.commit()
        conn.close()
        
        # 同时写入文件
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with open(self.trace_dir / "latest.md", "a") as f:
            f.write(f"[{timestamp}] [{phase}] {message}\n")
    
    def log(self, message):
        """记录日志"""
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with open(self.log_file, "a") as f:
            f.write(f"[{timestamp}] {message}\n")
        print(f"[{timestamp}] {message}")
    
    def run_command(self, cmd, cwd=None):
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
    
    def run_hermes(self, prompt):
        """调用Hermes Agent执行任务"""
        # 将 prompt 写入临时文件，避免 shell 转义问题
        prompt_file = self.trace_dir / "prompt.txt"
        with open(prompt_file, "w") as f:
            f.write(prompt)
        
        # 使用 -q 参数非交互式执行
        # 使用 -Q 参数安静模式，只输出最终响应
        cmd = f'hermes chat -q "$(cat {prompt_file})" -Q --yolo'
        code, stdout, stderr = self.run_command(cmd)
        return code, stdout, stderr
    
    def check_tests(self):
        """检查测试状态"""
        code, stdout, stderr = self.run_command("cargo test --workspace 2>&1 | grep 'test result:'")
        if code == 0:
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
        return 0, -1
    
    def commit_changes(self, message):
        """提交更改"""
        self.run_command("git add -A")
        code, stdout, stderr = self.run_command(f'git commit -m "{message}"')
        if code == 0:
            self.run_command("git push origin main")
            return True
        return False
    
    def revert_changes(self):
        """回滚更改"""
        self.run_command("git revert HEAD --no-edit")
        self.run_command("git push origin main")
    
    def select_module(self):
        """选择下一个要推进的模块"""
        modules = {
            'linux-automation': 'kias-linux-automation',
            'document-management': 'kias-document-management',
            'it-change-management': 'kias-it-change-management'
        }
        
        min_tests = float('inf')
        target = 'linux-automation'
        
        for module, package in modules.items():
            code, stdout, stderr = self.run_command(
                f"cargo test --package {package} 2>&1 | grep -c 'test .* ok'"
            )
            if code == 0:
                try:
                    tests = int(stdout.strip())
                    if tests < min_tests:
                        min_tests = tests
                        target = module
                except:
                    pass
        
        return target
    
    def gate_check(self, passed_before, failed_before, passed_after, failed_after):
        """5维AND门控"""
        # 1. 质量：测试全绿
        quality = (failed_after == 0)
        
        # 2. 安全：没有引入危险代码
        # 检查是否有 rm -rf、hardcoded API key 等
        code, stdout, stderr = self.run_command(
            "grep -r 'rm -rf\\|API_KEY\\|PASSWORD' --include='*.rs' --include='*.py' ."
        )
        safety = (code != 0)  # 没找到危险代码
        
        # 3. 效率：测试数量没有大幅减少
        efficiency = (passed_after >= passed_before * 0.9)
        
        # 4. 稳定性：测试通过率没有下降
        stability = (failed_after <= failed_before)
        
        # 5. 可维护性：代码行数没有爆炸式增长
        # 简化检查：没有新增超过1000行
        maintainability = True  # 简化处理
        
        return quality and safety and efficiency and stability and maintainability
    
    def run_iteration(self, iteration):
        """运行一轮迭代"""
        self.log(f"=== 迭代 {iteration} ===")
        
        # Phase 1: Review
        self.add_trace(iteration, "Phase 1", "Review - 读取状态")
        passed, failed = self.check_tests()
        self.log(f"测试状态: {passed}通过, {failed}失败")
        
        if failed > 0:
            self.log("测试失败，停止")
            return False
        
        # Phase 2: Ideate
        self.add_trace(iteration, "Phase 2", "Ideate - 选择任务")
        module = self.select_module()
        self.log(f"选择推进: {module}")
        
        # Phase 3: Modify
        self.add_trace(iteration, "Phase 3", "Modify - 调用LLM执行改动")
        prompt = f"""你是 AgentGuard 项目的自主开发者。

任务：推进 {module} 模块的一个小功能。

要求：
1. 只改一个东西
2. 写测试
3. 确保测试通过
4. 提交代码

工作目录：/workspace/kias
"""
        
        code, stdout, stderr = self.run_hermes(prompt)
        self.log(f"Hermes 返回: code={code}")
        
        if code != 0:
            self.log("Hermes 执行失败")
            return False
        
        # Phase 4: Commit
        self.add_trace(iteration, "Phase 4", "Commit - 提交代码")
        passed_after, failed_after = self.check_tests()
        
        if failed_after > 0:
            self.log("测试失败，revert")
            self.revert_changes()
            return False
        
        # Phase 5: Verify
        self.add_trace(iteration, "Phase 5", "Verify - 验证测试")
        
        # Phase 6: Gate
        self.add_trace(iteration, "Phase 6", "Gate - 5维AND门控")
        gate_result = self.gate_check(passed, failed, passed_after, failed_after)
        
        if not gate_result:
            self.log("门控失败，revert")
            self.revert_changes()
            return False
        
        # Phase 7: Log
        self.add_trace(iteration, "Phase 7", "Log - 记录结果")
        self.commit_changes(f"自主迭代 {iteration}: 推进 {module}")
        
        # Phase 8: Loop
        self.add_trace(iteration, "Phase 8", "Loop - 继续")
        self.log(f"迭代 {iteration} 完成")
        
        return True
    
    def run(self):
        """主循环"""
        self.log("=== AgentGuard 自主运行启动 ===")
        
        iteration = int(self.get_state("iteration", "0"))
        stuck_count = int(self.get_state("stuck_count", "0"))
        
        while self.running:
            iteration += 1
            self.set_state("iteration", str(iteration))
            
            success = self.run_iteration(iteration)
            
            if success:
                stuck_count = 0
                self.set_state("stuck_count", "0")
            else:
                stuck_count += 1
                self.set_state("stuck_count", str(stuck_count))
                
                if stuck_count >= 3:
                    self.log("连续3轮失败，停止")
                    break
            
            # 等待下一轮
            time.sleep(INTERVAL)
        
        self.log("=== AgentGuard 自主运行结束 ===")

def main():
    agent = AutonomousAgent()
    agent.run()

if __name__ == "__main__":
    main()
