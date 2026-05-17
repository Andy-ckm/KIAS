# skill-mcp：技能管理平台分析

> 来源：微信公众号 2026-05-18
> 用途：KIAS skill 系统设计参考

## 一、核心定位

skill-mcp = MCP 服务器，提供"技能文件系统 + 权限网关 + 编排引擎"三位一体的技能管理平台。

## 二、五个核心问题

| 问题 | skill-mcp 解法 | KIAS 现状 | 差距 |
|------|----------------|-----------|------|
| 技能散乱 | 集中存储 + manifest.json + 全文搜索 | skill registry 有基础 | **缺 manifest 元数据** |
| 版本控制 | SemVer + 内容哈希 + 一键回滚 | 无 | **缺版本控制** |
| 多技能编排 | YAML DAG + depends_on + 并行执行 | workflow-engine 有 DAG | **缺 YAML 声明式** |
| 权限管理 | 标签 RBAC (skill.tags ∩ user.tags) | auth + RBAC 已有 | ✅ 已有 |
| 安全检测 | prompt 注入检测 + lint | 无 | **缺安全检测** |

## 三、技术架构

```
src/
├── mcp/          # MCP 服务器 & 工具注册
├── services/     # 技能管理服务（核心业务逻辑）
├── pipeline/     # DAG 编排引擎
├── permission/   # RBAC 权限系统
└── storage/      # 数据持久化（SQLite + Drizzle ORM）
```

## 四、KIAS 借鉴

### 4.1 Skill Manifest（元数据）

```json
{
  "name": "github-pr-reader",
  "version": "1.2.0",
  "description": "Read GitHub PR content",
  "tags": ["github", "code-review"],
  "entry": "main.rs",
  "dependencies": ["github-api"],
  "permissions": ["network"]
}
```

KIAS 映射：扩展 SkillDef 结构体，加 version、tags、dependencies、permissions。

### 4.2 DAG 声明式编排

```yaml
name: code-review-pipeline
stages:
  - name: read-pr
    skill: github-pr-reader
    inputs: { url: "${inputs.pr_url}" }
  - name: security-scan
    skill: security-scanner
    depends_on: [read-pr]
  - name: generate-report
    skill: report-writer
    depends_on: [read-pr, security-scan]
```

KIAS 映射：workflow-engine 已有 DAG，需加 YAML 声明式加载。

### 4.3 Prompt 注入检测

skill-mcp 内置 lint 检测 prompt 注入。

KIAS 映射：skills crate 加安全检查器。

## 五、开发任务

1. [ ] Skill Manifest 元数据扩展（优先级：高）
2. [ ] Skill 版本控制 + rollback（优先级：高）
3. [ ] YAML DAG 声明式编排（优先级：中）
4. [ ] Skill 安全检查器（prompt 注入检测）（优先级：中）
5. [ ] Skill 标签搜索 + 过滤（优先级：中）
