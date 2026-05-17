# KIAS NL 输入界面 — 竞品参考分析

> 日期: 2026-05-16
> 参考源码: /mnt/reference-projects/{dify, coze-studio, assistant-ui}

## 1. 竞品概览

| 项目 | ⭐ Stars | 技术栈 | 定位 | 前端特点 |
|------|---------|--------|------|---------|
| **Dify** | 141K | Next.js + React + TS + Tailwind + Zustand | Agentic workflow 平台 | 最完整的 chat 组件体系 |
| **Coze Studio** | 20K | React + Semi Design + Zustand + Rsbuild | AI Agent 开发平台 | 135+ package monorepo，Agent IDE |
| **assistant-ui** | 10K | React + TypeScript + Radix-style primitives | AI Chat UI 组件库 | 无样式原语，最大灵活度 |
| **Langflow** | 148K | Python + React | Agent 可视化构建 | 拖拽式 workflow builder |
| **Open WebUI** | 137K | Python + Svelte | Chat 界面 | 类 ChatGPT 界面 |
| **Flowise** | 52K | TypeScript + React | 可视化 Agent 构建 | 拖拽式 chatflow |
| **FastGPT** | 28K | TypeScript + Next.js | 知识库平台 | 完整的 chat + 知识管理 |
| **RAGFlow** | 80K | Python + React | RAG 引擎 | 文档对话界面 |

## 2. Dify 前端架构（最值得参考）

### 技术选型
- Next.js 14+ App Router
- React 18 + TypeScript
- Tailwind CSS
- zustand（状态管理）
- react-textarea-autosize（输入框）
- Lexical（富文本编辑器）

### Chat 组件结构
```
web/app/components/base/chat/
├── chat/                           # 核心聊天组件
│   ├── index.tsx                   # 主组件：消息列表 + 输入区
│   ├── chat-input-area/index.tsx   # *** 输入框（textarea + 发送 + 文件 + 语音）***
│   ├── answer/                     # 回答气泡
│   │   ├── basic-content.tsx       # 纯文本/Markdown
│   │   ├── agent-content.tsx       # Agent 风格回答
│   │   ├── workflow-process.tsx    # 工作流执行展示
│   │   ├── suggested-questions.tsx # 回答后的建议问题
│   │   ├── human-input-content/    # 工作流中的人工输入表单
│   │   ├── citation/               # 引用渲染
│   │   ├── thought/                # Agent 推理过程
│   │   └── tool-detail.tsx         # 工具调用详情
│   ├── question.tsx                # 用户消息气泡
│   └── try-to-ask.tsx              # "试试问" 建议
├── chat-with-history/              # 带历史的完整 chatbot
│   ├── sidebar/                    # 会话历史侧边栏
│   └── header/                     # chatbot 头部
└── embedded-chatbot/               # 可嵌入版本
```

### NL 输入核心实现
```typescript
// chat-input-area/index.tsx
// - react-textarea-autosize 自动扩展 textarea
// - Enter 发送（支持 CJK 输入法 composition 感知）
// - 上下箭头浏览历史消息
// - 拖拽/粘贴文件上传
// - 语音输入（js-audio-recorder）
```

### KIAS 可借鉴
1. **Chat 组件三层结构**：核心聊天 → 带历史 → 可嵌入
2. **输入框**：textarea + 文件 + 语音 + 命令历史
3. **Agent 回答**：推理过程、工具调用详情、引用
4. **工作流展示**：workflow-process.tsx 展示执行进度
5. **建议问题**：suggested-questions 回答后推荐下一步

## 3. Coze Studio 前端架构（Agent IDE 参考）

### 技术选型
- React 18 + TypeScript
- Semi Design（字节跳动 UI 库）
- Zustand + Rsbuild + Rush.js monorepo
- 135+ 前端 package

### Agent IDE 布局（最值得参考）
```
┌─────────────────────────────────────────────────────┐
│  Agent IDE — 分屏布局                                  │
│                                                      │
│  ┌──────────────────┐  ┌──────────────────────────┐ │
│  │ Agent 配置区 (左)  │  │ Chat 预览区 (右)          │ │
│  │                  │  │                          │ │
│  │ - Prompt 编辑器   │  │ - 实时对话测试            │ │
│  │ - 模型选择       │  │ - 消息历史               │ │
│  │ - 工具/插件配置   │  │ - 工具调用展示            │ │
│  │ - 技能管理       │  │ - 推理过程               │ │
│  │ - Onboarding    │  │                          │ │
│  │                  │  │                          │ │
│  └──────────────────┘  └──────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

### KIAS 可借鉴
1. **分屏 Agent IDE**：左边配置，右边实时预览
2. **Adapter 模式**：所有组件通过 adapter 解耦
3. **Bot Detail Store**：统一的 Agent 状态管理
4. **Workflow Mode**：workflow-as-agent 模式

## 4. assistant-ui（组件库参考）

### 架构特点
- **无样式原语**（Radix 风格）：只提供逻辑，不提供样式
- **18 个原语命名空间**：Thread, Message, Composer, ActionBar, etc.
- **Runtime 适配器**：支持 Vercel AI SDK, LangChain, LangGraph, 本地
- **6 个模板**：default, minimal, cloud, langgraph, mcp

### 核心原语
```
ThreadPrimitive.Root           # 顶层容器
  ThreadPrimitive.Viewport     # 可滚动区域
    ThreadPrimitive.Messages   # 消息列表
      MessagePrimitive.Root    # 单条消息
        MessagePrimitive.Content
          MessagePartPrimitive.Text / .Image
        ActionBarPrimitive.Root
          .Copy / .Edit / .Reload / .Speak
    ThreadPrimitive.Suggestions # 建议问题
  ComposerPrimitive.Root       # 输入表单
    ComposerPrimitive.Input    # textarea
    ComposerPrimitive.Send     # 发送按钮
    ComposerPrimitive.Dictate  # 语音输入
```

### KIAS 可借鉴
1. **无样式原语**：最大灵活度，可以用自己的设计系统
2. **Runtime 适配器**：支持多种 LLM 后端
3. **工具系统**：makeAssistantTool / makeAssistantToolUI
4. **模板系统**：快速启动不同场景

## 5. KIAS NL 界面设计方案

### 推荐技术栈
- **React 18 + TypeScript + Tailwind CSS**（与 Dify 对齐）
- **assistant-ui 组件库**（无样式原语，最大灵活度）
- **Zustand**（状态管理）
- **react-textarea-autosize**（输入框）

### 页面结构
```
KIAS Dashboard
├── /                           # 首页 — NL 输入入口
│   └── ChatInput               # 大输入框："描述你的需求..."
│       ├── 建议卡片            # "创建 Agent" / "构建 Workflow" / "查看状态"
│       └── 历史记录
│
├── /chat/:conversation_id      # 对话页面
│   ├── 消息列表                # 用户消息 + 系统回答
│   ├── Agent Shell 推荐        # 匹配到的 Shell 卡片
│   ├── 参数确认表单            # 待确认的参数
│   ├── Workflow 预览           # 自动生成的 DAG 预览
│   └── 输入框                  # 持续对话
│
├── /agents                     # Agent 管理
│   ├── Shell 市场              # 浏览/搜索 Shell
│   └── 运行中 Agent            # 实例列表
│
├── /workflows                  # Workflow 管理
│   ├── DAG 编辑器              # 可视化编辑
│   └── 执行历史
│
└── /settings                   # 设置
    ├── Shell 管理              # 上传/编辑 Shell
    └── 系统配置
```

### 首页 NL 输入设计
参考 Dify 的 chat-with-history + Coze 的 Agent IDE：

```
┌──────────────────────────────────────────────────────────┐
│  KIAS — 智能体调度平台                                     │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │                                                    │  │
│  │     描述你的需求，KIAS 自动为你组装智能体...          │  │
│  │                                                    │  │
│  │     ┌────────┐  ┌────────┐  ┌────────┐            │  │
│  │     │创建 Agent│ │构建流程 │  │查看状态 │            │  │
│  │     └────────┘  └────────┘  └────────┘            │  │
│  │                                                    │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │ 📎 上传文件  🎤 语音  ⏎ 发送                        │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  最近对话:                                                │
│  · "帮我审查 kias 代码安全性"  2分钟前                    │
│  · "创建一个文档翻译 Agent"   1小时前                     │
│  · "分析竞品 Dify 的架构"     昨天                        │
└──────────────────────────────────────────────────────────┘
```

## 6. 实现优先级

### Phase 1: NL 输入组件（基于 assistant-ui）
- [ ] 安装 @assistant-ui/react + @assistant-ui/react-ai-sdk
- [ ] 自定义 Thread/Message/Composer 组件
- [ ] 接入 KIAS NL API

### Phase 2: Agent Shell 技能
- [ ] Shell YAML schema
- [ ] Shell 注册表
- [ ] 意图识别 → Shell 匹配 → 参数提取

### Phase 3: 对话页面
- [ ] Shell 推荐卡片
- [ ] 参数确认表单
- [ ] Workflow DAG 预览

### Phase 4: Dashboard 集成
- [ ] 首页 NL 输入入口
- [ ] Agent/Worflow 管理页面
- [ ] 与现有 React dashboard 集成
