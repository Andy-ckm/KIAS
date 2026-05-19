#!/usr/bin/env python3
"""
AgentGuard 自主开发代理

核心思想：
- 程序掌握控制流，LLM只管单点生成
- 没有trace证据不许动手
- 一次只改一个东西
- 先commit再验证
- 测试失败就revert
"""

import subprocess
import json
import os
from datetime import datetime
from pathlib import Path

PROJECT_DIR = Path("/workspace/kias")
TRACE_DIR = PROJECT_DIR / ".trace"
LOG_FILE = PROJECT_DIR / ".dev-log"
STATE_FILE = PROJECT_DIR / ".dev-state"

def run_command(cmd, cwd=None):
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

def check_tests():
    """检查测试状态"""
    code, stdout, stderr = run_command("cargo test --workspace 2>&1 | grep 'test result:'")
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

def write_trace(message):
    """写入trace"""
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    with open(TRACE_DIR / "latest.md", "a") as f:
        f.write(f"[{timestamp}] {message}\n")

def write_log(message):
    """写入日志"""
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    with open(LOG_FILE, "a") as f:
        f.write(f"[{timestamp}] {message}\n")

def commit_changes(message):
    """提交更改"""
    run_command("git add -A")
    code, stdout, stderr = run_command(f'git commit -m "{message}"')
    if code == 0:
        run_command("git push origin main")
        return True
    return False

def revert_changes():
    """回滚更改"""
    run_command("git revert HEAD --no-edit")
    run_command("git push origin main")

def select_module():
    """选择下一个要推进的模块"""
    modules = {
        'linux-automation': 'kias-linux-automation',
        'document-management': 'kias-document-management',
        'it-change-management': 'kias-it-change-management'
    }
    
    min_tests = float('inf')
    target = 'linux-automation'
    
    for module, package in modules.items():
        code, stdout, stderr = run_command(f"cargo test --package {package} 2>&1 | grep -c 'test .* ok'")
        if code == 0:
            try:
                tests = int(stdout.strip())
                if tests < min_tests:
                    min_tests = tests
                    target = module
            except:
                pass
    
    return target

def main():
    """主循环"""
    write_log("=== 自主开发代理启动 ===")
    
    # Phase 1: Review
    write_log("Phase 1: Review")
    passed, failed = check_tests()
    write_log(f"测试状态: {passed}通过, {failed}失败")
    write_trace(f"测试状态: {passed}通过, {failed}失败")
    
    if failed > 0:
        write_log("测试失败，停止")
        return
    
    # Phase 2: Ideate
    write_log("Phase 2: Ideate")
    module = select_module()
    write_log(f"选择推进: {module}")
    
    # Phase 3: Modify
    write_log("Phase 3: Modify")
    # 这里应该调用LLM来执行具体的改动
    # 但目前只是模拟
    write_log("等待LLM执行改动...")
    
    # Phase 4: Commit
    write_log("Phase 4: Commit")
    # 这里应该提交代码
    # 但目前只是模拟
    
    # Phase 5: Verify
    write_log("Phase 5: Verify")
    # 这里应该验证测试
    
    # Phase 6: Gate
    write_log("Phase 6: Gate")
    # 这里应该检查5维门控
    
    # Phase 7: Log
    write_log("Phase 7: Log")
    
    # Phase 8: Loop
    write_log("Phase 8: Loop")
    write_log("=== 自主开发代理结束 ===")

if __name__ == "__main__":
    main()
