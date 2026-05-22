# MiniMax 主动监控机制设计

> 由MiniMax大模型生成

# AgentGuard 前端协调监控机制设计方案

## 一、架构概览

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         AgentGuard 监控体系                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐         │
│   │  Cron    │    │  Git     │    │  CI/CD   │    │  实时    │         │
│   │  30min   │    │  Hooks   │    │  Pipeline│    │  日志流  │         │
│   └────┬─────┘    └────┬─────┘    └────┬─────┘    └────┬─────┘         │
│        │               │               │               │                │
│        └───────────────┴───────────────┴───────────────┘                │
│                              │                                          │
│                              ▼                                          │
│                   ┌──────────────────┐                                  │
│                   │   监控数据湖      │                                  │
│                   │  (Metrics Pool)  │                                  │
│                   └────────┬─────────┘                                  │
│                            │                                            │
│        ┌───────────────────┼───────────────────┐                       │
│        ▼                   ▼                   ▼                       │
│   ┌─────────┐        ┌─────────┐        ┌─────────┐                   │
│   │ 自动分类 │        │ 智能修复 │        │ 报告生成 │                   │
│   │ P0-P3   │        │ Engine  │        │ Dashboard│                   │
│   └────┬────┘        └────┬────┘        └────┬────┘                   │
│        │                  │                  │                          │
│        └──────────────────┼──────────────────┘                          │
│                           ▼                                             │
│                   ┌──────────────────┐                                  │
│                   │   可视化 Harness │                                  │
│                   │   Integration    │                                  │
│                   └──────────────────┘                                  │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 二、监控范围（What to Monitor）

### 2.1 编译监控

```yaml
编译健康度:
  指标列表:
    - 编译错误数 (ErrorCount)
    - 编译警告数 (WarningCount)  
    - 增量编译时间 (IncrementalBuildTime)
    - 全量编译时间 (FullBuildTime)
    - crate 间依赖解析时间
    - 并行编译利用率 (ParallelUtilization)
    
  阈值配置:
    error_count:
      理想: 0
      P2警告: 1-5
      P1警报: 6-20
      P0紧急: >20
      
    build_time:
      增量编译: <30s (理想), >2min (需优化)
      全量编译: <5min (理想), >15min (P1)
```

### 2.2 测试监控

```rust
// 测试健康度指标定义
pub struct TestHealthMetrics {
    // 数量统计
    total_tests: u64,
    passed: u64,
    failed: u64,
    ignored: u64,
    compilation_errors: u64,
    
    // 时间统计  
    total_duration_ms: u64,
    slowest_tests: Vec<(String, Duration)>,
    flakiness_candidates: Vec<String>,  // 多次运行结果不一致
    
    // 覆盖度
    line_coverage: f64,
    branch_coverage: f64,
    
    // 按 crate 分组
    per_crate_status: HashMap<CrateName, CrateTestSummary>,
}

#[derive(PartialEq, Clone, Serialize)]
pub enum TestSeverity {
    CriticalPath,      // 关键路径测试
    Integration,       // 集成测试
    Unit,              // 单元测试
    Smoke,             // 冒烟测试
}
```

### 2.3 Clippy Lint 监控

```toml
[monitoring.clippy]
# 分级监控的 lint 规则

critical_lints = [
    "clippy::unwrap_used",
    "clippy::panic",
    "clippy::expect_used", 
    "clippy::unreachable",
    "clippy::todo",
]

warning_lints = [
    "clippy::clone_on_copy",
    "clippy::redundant_clone",
    "clippy::inefficient_to_string",
    "clippy::unnecessary_filter_map",
]

style_lints = [
    "clippy::module_name_repetitions",
    "clippy::similar_names",
    "clippy::too_many_lines",
]

perf_lints = [
    "clippy::useless_vec",
    "clippy::iter_skip_next",
    "clippy::or_fun_call",
]
```

### 2.4 Unwrap/panic 模式监控

```rust
// 危险操作监控
pub struct UnwrapMonitor {
    // 统计各 crate 中 unwrap/expect 使用情况
    unwrap_locations: HashMap<CratePath, Vec<UnwrapSite>>,
    
    // 风险等级
    risk_assessment: RiskLevel,
}

#[derive(Display, Debug)]
pub enum RiskLevel {
    Safe,      // unwrap() on Option<T> where T: Default, 或测试代码
    Low,       // unwrap() on 确定性操作结果
    Medium,    // unwrap() on 外部输入处理
    High,      // unwrap() on 网络/IO 结果
    Critical,  // unwrap() 在关键路径，缺少 fallback
}

// 检测规则
detection_rules = {
    "unwrap_used" => "在生产代码中使用 unwrap()",
    "expect_used" => "使用 expect() 而非 ? 操作符",
    "unwrap_or" => "推荐使用 unwrap_or() 或 unwrap_or_else()",
    "if_let_some" => "可替代 unwrap 的更安全模式",
}
```

### 2.5 安全监控

```yaml
安全扫描配置:
  依赖审计:
    - cargo-audit: 检查依赖中的已知漏洞
    - cargo-deny: 许可证和依赖策略合规
    - cargo-geiger: 检测不安全代码使用情况
    
  代码安全模式:
    - 硬编码凭证检测
    - SQL/命令注入风险
    - 序列化安全问题
    - 加密实现正确性
    
  运行时安全:
    - 内存安全 (ASan/MSan/UBsan)
    - 线程安全 (Miri 静态分析)
    - 资源泄漏 (FD 计数、内存分配)

警报规则:
  P0 - 立即处理:
    - 已知 CVE 漏洞
    - RCE 风险
    - 敏感信息泄露
    
  P1 - 24小时内:
    - DoS 风险
    - 权限提升可能
    - 数据篡改可能
    
  P2 - 计划修复:
    - 代码质量问题
    - 配置不当
    - 缺少安全最佳实践
```

### 2.6 性能监控

```rust
// 性能监控指标
pub struct PerfMetrics {
    // 编译性能
    compilation: CompilationPerf,
    
    // 测试性能
    test_execution: TestPerf,
    
    // 运行时性能 (如含二进制)
    runtime: RuntimePerf,
}

#[derive(Serialize)]
pub struct CompilationPerf {
    /// 增量编译时间 (目标 <30s)
    incremental_build_ms: u64,
    /// 全量编译时间 (目标 <5min)  
    full_build_ms: u64,
    /// 内存峰值 (目标 <8GB)
    peak_memory_mb: u64,
    /// CPU 利用率
    cpu_utilization_pct: f64,
}

#[derive(Serialize)]  
pub struct TestPerf {
    /// 测试套件总时间 (目标 <10min)
    total_duration_ms: u64,
    /// 平均单测试时间
    avg_test_duration_us: u64,
    /// 慢测试 (>1s) 列表
    slow_tests: Vec<SlowTest>,
    /// 可能的性能回归
    regressions: Vec<Regression>,
}

pub struct SlowTest {
    test_path: String,
    duration_ms: u64,
    baseline_ms: u64,    // 基线时间
    threshold_ms: u64,   // 阈值
    trend: Trend,        // 上升/下降/稳定
}

#[derive(PartialEq)]
pub enum Trend {
    Improving,
    Stable,
    Degrading,
    Unknown,
}
```

---

## 三、捕获机制（How to Capture）

### 3.1 定时任务捕获 (Cron 30min)

```yaml
# .github/workflows/scheduled_monitor.yml
name: 定时健康检查

on:
  schedule:
    - cron: '*/30 * * * *'  # 每30分钟
  
  # 也支持手动触发
  workflow_dispatch:
    inputs:
      full_scan:
        description: '完整扫描'
        type: boolean
        default: false

jobs:
  health_check:
    runs-on: self-hosted
    
    steps:
      - uses: actions/checkout@v4
        
      - name: 编译健康检查
        run: |
          cargo build --all-targets 2>&1 | tee build.log
          python scripts/parse_build_log.py --input build.log --output build_metrics.json
          
      - name: 测试健康检查
        run: |
          cargo test --all --no-fail-fast --timing-json
          python scripts/parse_test_timing.py --output test_metrics.json
          
      - name: Clippy 检查
        run: |
          cargo clippy --all --message-format=json > clippy_report.json
          python scripts/analyze_clippy.py --input clippy_report.json
          
      - name: 安全扫描
        run: |
          cargo audit --json > audit_report.json
          cargo-geiger --json > geiger_report.json
          
      - name: 上报监控数据
        run: |
          python scripts/report_metrics.py \
            --build build_metrics.json \
            --test test_metrics.json \
            --clippy clippy_report.json \
            --security audit_report.json \
            --destination s3://agentguard-metrics/
```

### 3.2 Git Hooks 捕获

```bash
#!/bin/bash
# .git/hooks/pre-commit

set -e

echo "🔍 运行 pre-commit 检查..."

# 1. 只检查变更文件
CHANGED_FILES=$(git diff --cached --name-only --diff-filter=ACM)
STAGED_CRATES=$(echo "$CHANGED_FILES" | xargs -I{} dirname {} | sort -u | grep -E '^crates/')

# 2. 快速 lint 检查
echo "📋 运行 Clippy..."
cargo clippy --all-features -- -D warnings 2>&1 | head -100

# 3. 检查 unwrap 使用 (新增代码中禁止 unwrap)
echo "🚫 检查危险 unwrap 使用..."
python scripts/check_unsafe_patterns.py --files "$CHANGED_FILES"

# 4. 格式化检查
echo "📐 格式化检查..."
cargo fmt --all -- --check

# 5. 测试变更相关 crate
echo "🧪 运行相关测试..."
cargo test -p $(echo "$STAGED_CRATES" | tr '\n' ' ') --no-run

echo "✅ Pre-commit 检查完成"
```

```bash
#!/bin/bash  
# .git/hooks/pre-push

echo "🔬 运行 pre-push 深度检查..."

# 1. 完整编译
echo "🏗️  全量编译..."
cargo build --release --all-targets

# 2. 全量测试 (允许并行)
echo "🧪 全量测试..."
cargo test --all -- --test-threads=$(nproc)

# 3. 完整 clippy
echo "📋 完整 Clippy..."
cargo clippy --all -- -D clippy::correctness -D clippy::suspicious

echo "✅ Pre-push 检查完成"
```

### 3.3 CI/CD Pipeline 集成

```yaml
# .github/workflows/pr_checks.yml
name: PR 检查流水线

on:
  pull_request:
    branches: [main, develop]
    
concurrency:
  group: pr-${{ github.event.pull_request.number }}
  cancel-in-progress: true

jobs:
  # 阶段1: 快速检查 (<5min)
  quick_checks:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@v4
        
      - name: 变更检测
        id: changed
        uses: ./.github/actions/detect-changes
        
      - name: 仅编译变更 crate
        if: steps.changed.outputs.crates != ''
        run: |
          for crate in ${{ steps.changed.outputs.crates }}; do
            cargo build -p "$crate"
          done
          
      - name: Clippy 快速模式
        run: cargo clippy --all-features -- -D warnings