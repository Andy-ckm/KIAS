#!/usr/bin/env python3
"""KIAS Task Queue - 文件级任务队列管理"""
import json, os, time, sys
from pathlib import Path

QUEUE_DIR = Path("/workspace/kias/.task-queue")
QUEUE_FILE = QUEUE_DIR / "pending.jsonl"
DONE_FILE = QUEUE_DIR / "done.jsonl"
LOCK_FILE = QUEUE_DIR / ".lock"

def enqueue(task_type: str, payload: dict, priority: int = 5):
    """入队一个任务"""
    task = {
        "id": f"task_{int(time.time()*1000)}",
        "type": task_type,
        "payload": payload,
        "priority": priority,
        "created_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "status": "pending"
    }
    with open(QUEUE_FILE, "a") as f:
        f.write(json.dumps(task, ensure_ascii=False) + "\n")
    return task["id"]

def dequeue(limit: int = 1):
    """出队待处理任务（按优先级排序）"""
    if not QUEUE_FILE.exists():
        return []
    
    with open(QUEUE_FILE, "r") as f:
        tasks = [json.loads(line) for line in f if line.strip()]
    
    tasks.sort(key=lambda t: t.get("priority", 5))
    taken = tasks[:limit]
    remaining = tasks[limit:]
    
    with open(QUEUE_FILE, "w") as f:
        for t in remaining:
            f.write(json.dumps(t, ensure_ascii=False) + "\n")
    
    return taken

def complete(task_id: str, result: dict):
    """标记任务完成"""
    entry = {"task_id": task_id, "result": result, "completed_at": time.strftime("%Y-%m-%d %H:%M:%S")}
    with open(DONE_FILE, "a") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")

def stats():
    """查看队列状态"""
    pending = 0
    done = 0
    if QUEUE_FILE.exists():
        with open(QUEUE_FILE) as f:
            pending = sum(1 for _ in f)
    if DONE_FILE.exists():
        with open(DONE_FILE) as f:
            done = sum(1 for _ in f)
    return {"pending": pending, "done": done}

if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "stats"
    if cmd == "enqueue":
        task_type = sys.argv[2]
        payload = json.loads(sys.argv[3])
        tid = enqueue(task_type, payload)
        print(f"Enqueued: {tid}")
    elif cmd == "dequeue":
        tasks = dequeue()
        print(json.dumps(tasks, ensure_ascii=False, indent=2))
    elif cmd == "stats":
        print(json.dumps(stats()))
    elif cmd == "complete":
        complete(sys.argv[2], {"status": "done"})
        print(f"Completed: {sys.argv[2]}")
