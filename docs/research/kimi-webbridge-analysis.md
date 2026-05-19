# Kimi WebBridge：Agent 浏览器自动化分析

> 来源：微信公众号 2026-05-18
> 用途：AgentGuard web 工具层设计参考

## 一、核心能力

| 能力 | 描述 | AgentGuard 映射 |
|------|------|-----------|
| 搜索 | 跨平台搜索（X、Reddit、HN） | web-search 工具 |
| 滚动/点击/输入 | 模拟人类操作 | browser 工具 |
| 页面切换 | 多标签页管理 | browser session |
| 网页复刻 | 1:1 复制网页布局 | 需要多模态 |
| 表单创建 | 自动填写 Google 表单 | browser automation |
| 数据聚合 | 跨平台汇总成表格 | data-store |
| 工作流→Skill | 浏览器操作变成可复用技能 | skill 自动生成 |

## 二、使用场景

1. **热帖抓取**：X + Reddit + HN 跨平台搜索，自动汇总表格
2. **招聘信息**：LinkedIn 搜索 + 筛选 + 导出 Excel
3. **商品比价**：Google Shopping + Amazon 横向对比
4. **网页复刻**：打开网页 → 复制布局 → 生成新页面
5. **表单创建**：Google Forms 自动构建调查问卷
6. **日常工作流**：天气查询等重复操作 → Skill

## 三、架构启示

**开放生态**：WebBridge 不只服务 Kimi，兼容 Claude Code、Codex、Cursor、Hermes。

**AgentGuard 借鉴**：
1. web-browser 工具需要支持 MCP 协议（标准接口）
2. 浏览器操作结果需要结构化（不只是截图）
3. 工作流录制 → Skill 自动生成
4. 跨平台数据聚合需要统一格式

## 四、开发任务

1. [x] MCP browser 工具接入（优先级：高）
2. [ ] 跨平台数据聚合框架（优先级：中）
3. [ ] 浏览器工作流录制→Skill（优先级：中）
4. [ ] 网页内容结构化提取（优先级：中）
