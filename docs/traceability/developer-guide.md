# KIAS 开发者维护指南

> 为后期开发者提供完整的维护和开发指南

## 快速开始

### 环境准备
```bash
# 1. 克隆仓库
git clone <repository-url>
cd kias

# 2. 安装Rust（如果未安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
rustup update

# 3. 配置Cargo镜像（中国用户）
mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << EOF
[source.crates-io]
replace-with = 'ustc'

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
EOF

# 4. 构建项目
cargo build --workspace

# 5. 运行测试
cargo test --workspace -- --nocapture
```

### 项目结构
```
kias/
├── crates/                 # 核心模块
│   ├── model-router/      # 模型路由和Key轮转
│   ├── team-engine/       # 工作空间和会话管理
│   ├── common/            # 基础工具库
│   ├── mcp-protocol/      # MCP协议实现
│   ├── controller/        # 任务控制器
│   └── scheduler/         # 任务调度器
├── docs/                  # 文档
│   ├── adr/              # 架构决策记录
│   ├── features/         # 特性文档
│   └── traceability/     # 可追溯性文档
├── tests/                 # 集成测试
└── reference-projects/   # 参考源码
```

## 开发流程

### 1. 新特性开发
```bash
# 1. 创建特性分支
git checkout -b feature/xxx

# 2. 创建ADR文档
# 编辑 docs/adr/ADR-XXX-xxx.md

# 3. 实现代码
# 编辑 crates/xxx/src/xxx.rs

# 4. 编写测试
# 编辑 crates/xxx/tests/xxx.rs

# 5. 更新文档
# 编辑 docs/features/xxx.md

# 6. 运行测试
cargo test --workspace -- --nocapture

# 7. 提交代码
git add .
git commit -m "feat(xxx): 实现xxx特性"

# 8. 创建Pull Request
gh pr create --title "feat(xxx): 实现xxx特性" --body "..."
```

### 2. Bug修复流程
```bash
# 1. 创建修复分支
git checkout -b fix/xxx

# 2. 定位问题
cargo test --package xxx -- --nocapture 2>&1 | grep -A5 -B5 "FAILED"

# 3. 修复问题
# 编辑相关代码

# 4. 编写回归测试
# 确保问题不再复现

# 5. 运行完整测试
cargo test --workspace -- --nocapture

# 6. 提交修复
git commit -m "fix(xxx): 修复xxx问题"
```

### 3. 重构流程
```bash
# 1. 创建重构分支
git checkout -b refactor/xxx

# 2. 记录重构原因
# 编辑 docs/adr/ADR-XXX-xxx.md

# 3. 渐进式重构
# 小步重构，每步都运行测试

# 4. 性能基准测试
cargo bench

# 5. 更新文档
# 编辑相关文档

# 6. 提交重构
git commit -m "refactor(xxx): 重构xxx模块"
```

## 测试策略

### 测试类型
1. **单元测试**：测试单个函数或方法
2. **集成测试**：测试模块间交互
3. **端到端测试**：测试完整流程

### 测试命令
```bash
# 运行所有测试
cargo test --workspace

# 运行特定模块测试
cargo test --package model-router

# 运行特定测试用例
cargo test --package model-router key_rotation

# 生成测试覆盖率报告
cargo tarpaulin --workspace --out Html

# 运行基准测试
cargo bench
```

### 测试最佳实践
1. **测试命名**：清晰描述测试场景
2. **测试隔离**：测试间无共享状态
3. **测试数据**：使用内存文件系统
4. **断言清晰**：明确的错误信息

## 代码质量

### Clippy检查
```bash
# 运行Clippy检查
cargo clippy --workspace -- -D warnings

# 自动修复Clippy警告
cargo clippy --workspace --fix
```

### 代码格式化
```bash
# 格式化代码
cargo fmt --all

# 检查格式
cargo fmt --all -- --check
```

### 文档生成
```bash
# 生成API文档
cargo doc --workspace --open

# 检查文档链接
cargo doc --workspace --no-deps
```

## 架构维护

### 新增Crate
```bash
# 1. 创建新Crate
cargo new crates/xxx --lib

# 2. 添加依赖
# 编辑 Cargo.toml

# 3. 更新workspace依赖
# 编辑 根目录 Cargo.toml

# 4. 实现功能
# 编辑 crates/xxx/src/lib.rs

# 5. 编写测试
# 编辑 crates/xxx/tests/xxx.rs

# 6. 更新文档
# 编辑 docs/features/xxx.md
```

### 依赖管理
```bash
# 查看依赖树
cargo tree

# 检查依赖更新
cargo outdated

# 更新依赖
cargo update
```

### 性能优化
```bash
# 生成火焰图
cargo flamegraph --bin kias

# 基准测试
cargo bench

# 内存分析
valgrind --tool=massif target/debug/kias
```

## 故障排查

### 常见问题

#### 1. 编译错误
```bash
# 清理构建缓存
cargo clean

# 重新构建
cargo build --workspace
```

#### 2. 测试失败
```bash
# 运行失败测试并查看详细输出
cargo test --package xxx -- --nocapture 2>&1 | tail -20

# 运行特定测试
cargo test --package xxx test_name -- --nocapture
```

#### 3. 性能问题
```bash
# 分析编译时间
cargo build --timings

# 分析运行时性能
cargo flamegraph --bin kias
```

#### 4. 内存问题
```bash
# 检查内存泄漏
valgrind --leak-check=full target/debug/kias

# 检查内存使用
heaptrack target/debug/kias
```

## 文档维护

### 文档更新流程
1. **代码变更后**：立即更新相关文档
2. **Sprint结束**：更新开发日志和变更日志
3. **版本发布**：更新版本号和发布说明
4. **架构变更**：更新架构决策记录

### 文档质量标准
1. **准确性**：文档必须与代码一致
2. **完整性**：覆盖所有重要功能
3. **清晰性**：语言简洁明了
4. **可维护性**：易于更新和扩展

## 部署指南

### 生产环境部署
```bash
# 1. 构建发布版本
cargo build --release

# 2. 配置环境变量
export KIAS_ENV=production
export KIAS_LOG_LEVEL=info

# 3. 运行服务
./target/release/kias --config config/production.toml
```

### 监控和告警
1. **日志监控**：ELK Stack
2. **指标监控**：Prometheus + Grafana
3. **告警设置**：基于指标阈值

### 备份策略
1. **数据库备份**：每日全量备份
2. **配置备份**：版本控制
3. **日志备份**：定期归档

## 社区贡献

### 贡献流程
1. **Fork仓库**
2. **创建特性分支**
3. **提交代码**
4. **创建Pull Request**
5. **代码审查**
6. **合并到主分支**

### 代码审查标准
1. **代码质量**：符合Rust最佳实践
2. **测试覆盖**：新功能必须包含测试
3. **文档完整**：新功能必须包含文档
4. **性能影响**：评估性能影响

### 发布流程
1. **版本号更新**：遵循语义化版本
2. **变更日志更新**：记录所有变更
3. **发布说明**：详细的发布说明
4. **Git标签**：创建版本标签
