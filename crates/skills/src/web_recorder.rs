//! # Browser Workflow Recorder → Skill Auto-Generation
//!
//! 灵感来源：Kimi WebBridge
//!
//! 将浏览器操作录制转化为可复用的 Skill，实现"录一遍，复用无限次"。
//!
//! ## 架构
//!
//! ```text
//! 用户浏览器操作 → WebRecorder 录制 → BrowserRecording
//!                                         ↓
//!                                   SkillGenerator
//!                                         ↓
//!                                  BrowserWorkflowSkill → 注册到 SkillRegistry
//! ```
//!
//! ## 设计原则
//!
//! 1. **参数化**：录制中的动态值（用户名、密码、搜索词）可提取为参数
//! 2. **幂等性**：同一 Skill 多次执行结果一致（带 Wait/Retry）
//! 3. **可组合**：生成的 Skill 可嵌入 Pipeline / CompositeSkill
//! 4. **可验证**：录制可导出为 JSON，支持 diff 比较和回归测试

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

use kias_common::{KiasError, KiasResult};

use crate::skill::{Skill, SkillConfig};

// ── Browser Action Types ──────────────────────────────────────────

/// 单个浏览器动作
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BrowserAction {
    /// 导航到 URL
    Navigate { url: String },
    /// 点击元素（CSS 选择器）
    Click { selector: String },
    /// 在输入框中输入文本
    Input {
        selector: String,
        value: String,
        /// 是否为参数化字段（运行时由外部传入）
        parameterized: bool,
    },
    /// 等待元素出现
    WaitForElement { selector: String, timeout_ms: u64 },
    /// 等待指定时间
    Wait { duration_ms: u64 },
    /// 截图（用于验证 / 调试）
    Screenshot { name: String },
    /// 从页面提取文本
    ExtractText {
        selector: String,
        output_key: String,
    },
    /// 提交表单
    Submit { selector: String },
    /// 按键操作
    KeyPress { key: String },
    /// 滚动页面
    Scroll {
        direction: ScrollDirection,
        amount: i32,
    },
    /// 执行 JavaScript
    EvaluateJs {
        script: String,
        output_key: Option<String>,
    },
    /// 断言：检查元素存在
    AssertElementExists { selector: String },
    /// 断言：检查文本内容
    AssertTextContains {
        selector: String,
        expected_text: String,
    },
}

/// 滚动方向
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

// ── Recording Metadata ────────────────────────────────────────────

/// 录制元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    /// 录制名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 录制时间
    pub recorded_at: SystemTime,
    /// 录制时长（毫秒）
    pub duration_ms: u64,
    /// 目标网站域名
    pub domain: String,
    /// 标签
    pub tags: Vec<String>,
    /// 录制者
    pub recorded_by: String,
}

/// 参数定义：将录制中的动态值提取为可配置参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDefinition {
    /// 参数名称
    pub name: String,
    /// 参数描述
    pub description: String,
    /// 参数类型
    pub param_type: ParameterType,
    /// 默认值
    pub default_value: Option<String>,
    /// 是否必填
    pub required: bool,
}

/// 参数类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    Url,
    Selector,
}

// ── Browser Recording ─────────────────────────────────────────────

/// 浏览器操作录制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserRecording {
    /// 录制 ID
    pub id: String,
    /// 录制元数据
    pub metadata: RecordingMetadata,
    /// 操作序列
    pub actions: Vec<RecordedAction>,
    /// 参数化定义
    pub parameters: Vec<ParameterDefinition>,
    /// 版本号
    pub version: String,
}

/// 带元数据的录制动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedAction {
    /// 动作序号
    pub step: usize,
    /// 浏览器动作
    pub action: BrowserAction,
    /// 动作描述（人类可读）
    pub description: String,
    /// 该步骤的截图（base64，可选）
    pub screenshot: Option<String>,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 是否关键步骤（失败时中断）
    pub critical: bool,
}

// ── Web Recorder ──────────────────────────────────────────────────

/// 浏览器工作流录制器
pub struct WebRecorder {
    /// 当前录制
    current_recording: Option<BrowserRecording>,
    /// 已完成的录制
    recordings: Vec<BrowserRecording>,
    /// 录制配置
    config: RecorderConfig,
}

/// 录制器配置
#[derive(Debug, Clone)]
pub struct RecorderConfig {
    /// 自动截图（每步）
    pub auto_screenshot: bool,
    /// 自动等待（每步后）
    pub auto_wait_ms: u64,
    /// 最大步骤数
    pub max_steps: usize,
    /// 录制名称前缀
    pub name_prefix: String,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            auto_screenshot: false,
            auto_wait_ms: 500,
            max_steps: 100,
            name_prefix: "recording".to_string(),
        }
    }
}

impl WebRecorder {
    /// 创建新的录制器
    pub fn new(config: RecorderConfig) -> Self {
        Self {
            current_recording: None,
            recordings: Vec::new(),
            config,
        }
    }

    /// 开始录制
    pub fn start_recording(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        domain: impl Into<String>,
    ) -> String {
        let name_str = name.into();
        let id = format!(
            "rec-{}-{}",
            name_str.replace(' ', "-").to_lowercase(),
            Self::timestamp_id()
        );

        let recording = BrowserRecording {
            id: id.clone(),
            metadata: RecordingMetadata {
                name: name_str,
                description: description.into(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: domain.into(),
                tags: Vec::new(),
                recorded_by: "web-recorder".to_string(),
            },
            actions: Vec::new(),
            parameters: Vec::new(),
            version: "1.0.0".to_string(),
        };

        self.current_recording = Some(recording);
        tracing::info!(recording_id = %id, "Recording started");
        id
    }

    /// 记录一个浏览器动作
    pub fn record_action(
        &mut self,
        action: BrowserAction,
        description: impl Into<String>,
        critical: bool,
    ) -> KiasResult<usize> {
        let recording = self.current_recording.as_mut().ok_or_else(|| {
            KiasError::Validation("No active recording. Call start_recording() first.".into())
        })?;

        if recording.actions.len() >= self.config.max_steps {
            return Err(KiasError::Validation(format!(
                "Max steps ({}) exceeded",
                self.config.max_steps
            )));
        }

        let step = recording.actions.len();
        let recorded = RecordedAction {
            step,
            action,
            description: description.into(),
            screenshot: None,
            duration_ms: self.config.auto_wait_ms,
            critical,
        };

        recording.actions.push(recorded);
        tracing::debug!(step = step, "Action recorded");
        Ok(step)
    }

    /// 添加参数化定义
    pub fn add_parameter(&mut self, param: ParameterDefinition) -> KiasResult<()> {
        let recording = self
            .current_recording
            .as_mut()
            .ok_or_else(|| KiasError::Validation("No active recording.".into()))?;

        recording.parameters.push(param);
        Ok(())
    }

    /// 停止录制并返回结果
    pub fn stop_recording(&mut self) -> KiasResult<BrowserRecording> {
        let mut recording = self
            .current_recording
            .take()
            .ok_or_else(|| KiasError::Validation("No active recording.".into()))?;

        // 计算总时长
        recording.metadata.duration_ms = recording.actions.iter().map(|a| a.duration_ms).sum();

        let result = recording.clone();
        self.recordings.push(recording);

        tracing::info!(
            recording_id = %result.id,
            steps = result.actions.len(),
            "Recording stopped"
        );

        Ok(result)
    }

    /// 获取所有已完成的录制
    pub fn recordings(&self) -> &[BrowserRecording] {
        &self.recordings
    }

    /// 从 JSON 反序列化录制
    pub fn from_json(json: &str) -> KiasResult<BrowserRecording> {
        serde_json::from_str(json)
            .map_err(|e| KiasError::Validation(format!("Invalid recording JSON: {}", e)))
    }

    /// 序列化录制为 JSON
    pub fn to_json(recording: &BrowserRecording) -> KiasResult<String> {
        serde_json::to_string_pretty(recording)
            .map_err(|e| KiasError::Validation(format!("Serialization error: {}", e)))
    }

    fn timestamp_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let dur = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}", dur.as_millis())
    }
}

// ── Skill Generator ───────────────────────────────────────────────

/// 将 BrowserRecording 转化为可执行 Skill 的生成器
pub struct SkillGenerator;

impl SkillGenerator {
    /// 从录制生成 BrowserWorkflowSkill
    pub fn generate_skill(recording: BrowserRecording) -> BrowserWorkflowSkill {
        let skill_name = format!(
            "web-{}",
            recording.metadata.name.replace(' ', "-").to_lowercase()
        );

        tracing::info!(
            skill_name = %skill_name,
            steps = recording.actions.len(),
            parameters = recording.parameters.len(),
            "Generating skill from recording"
        );

        BrowserWorkflowSkill {
            name: skill_name,
            recording,
            config: WorkflowConfig::default(),
        }
    }

    /// 从录制生成 SkillConfig
    pub fn generate_skill_config(recording: &BrowserRecording) -> SkillConfig {
        let skill_name = format!(
            "web-{}",
            recording.metadata.name.replace(' ', "-").to_lowercase()
        );

        let mut tags = recording.metadata.tags.clone();
        tags.push("browser".to_string());
        tags.push("workflow".to_string());
        tags.push("auto-generated".to_string());

        SkillConfig::new(
            &skill_name,
            format!(
                "Auto-generated from browser recording: {}. {} steps.",
                recording.metadata.name,
                recording.actions.len()
            ),
        )
        .with_tags(tags)
        .with_version(&recording.version)
    }

    /// 从 JSON 字符串生成 Skill（一步到位）
    pub fn from_json(json: &str) -> KiasResult<BrowserWorkflowSkill> {
        let recording = WebRecorder::from_json(json)?;
        Ok(Self::generate_skill(recording))
    }

    /// 参数替换：将录制中的占位符替换为实际参数值
    pub fn apply_parameters(
        recording: &BrowserRecording,
        params: &HashMap<String, String>,
    ) -> BrowserRecording {
        let mut result = recording.clone();

        for action in &mut result.actions {
            match &mut action.action {
                BrowserAction::Navigate { url } => {
                    *url = Self::replace_placeholders(url, params);
                }
                BrowserAction::Input { value, .. } => {
                    *value = Self::replace_placeholders(value, params);
                }
                BrowserAction::Click { selector } => {
                    *selector = Self::replace_placeholders(selector, params);
                }
                BrowserAction::WaitForElement { selector, .. } => {
                    *selector = Self::replace_placeholders(selector, params);
                }
                BrowserAction::ExtractText { selector, .. } => {
                    *selector = Self::replace_placeholders(selector, params);
                }
                BrowserAction::Submit { selector } => {
                    *selector = Self::replace_placeholders(selector, params);
                }
                BrowserAction::AssertElementExists { selector } => {
                    *selector = Self::replace_placeholders(selector, params);
                }
                BrowserAction::AssertTextContains {
                    selector,
                    expected_text,
                } => {
                    *selector = Self::replace_placeholders(selector, params);
                    *expected_text = Self::replace_placeholders(expected_text, params);
                }
                BrowserAction::EvaluateJs { script, .. } => {
                    *script = Self::replace_placeholders(script, params);
                }
                _ => {}
            }
        }

        result
    }

    /// 替换 {{param_name}} 占位符
    fn replace_placeholders(text: &str, params: &HashMap<String, String>) -> String {
        let mut result = text.to_string();
        for (key, value) in params {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }
}

// ── Workflow Execution Config ─────────────────────────────────────

/// 工作流执行配置
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    /// 每步超时（毫秒）
    pub step_timeout_ms: u64,
    /// 失败时重试次数
    pub retry_count: u32,
    /// 重试间隔（毫秒）
    pub retry_delay_ms: u64,
    /// 是否在断言失败时中断
    pub stop_on_assert_failure: bool,
    /// 截图目录
    pub screenshot_dir: Option<String>,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            step_timeout_ms: 30_000,
            retry_count: 2,
            retry_delay_ms: 1_000,
            stop_on_assert_failure: true,
            screenshot_dir: None,
        }
    }
}

// ── Workflow Execution Result ─────────────────────────────────────

/// 工作流执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 执行的步骤数
    pub steps_executed: usize,
    /// 总步骤数
    pub total_steps: usize,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
    /// 每步结果
    pub step_results: Vec<StepResult>,
    /// 提取的数据
    pub extracted_data: HashMap<String, String>,
    /// 错误信息
    pub error: Option<String>,
    /// 截图路径列表
    pub screenshots: Vec<String>,
}

/// 单步执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// 步骤序号
    pub step: usize,
    /// 动作描述
    pub description: String,
    /// 是否成功
    pub success: bool,
    /// 耗时（毫秒）
    pub duration_ms: u64,
    /// 错误信息
    pub error: Option<String>,
}

// ── BrowserWorkflowSkill ──────────────────────────────────────────

/// 从浏览器录制自动生成的 Skill
pub struct BrowserWorkflowSkill {
    /// Skill 名称
    name: String,
    /// 源录制
    recording: BrowserRecording,
    /// 执行配置
    config: WorkflowConfig,
}

impl BrowserWorkflowSkill {
    /// 创建新的 BrowserWorkflowSkill
    pub fn new(name: impl Into<String>, recording: BrowserRecording) -> Self {
        Self {
            name: name.into(),
            recording,
            config: WorkflowConfig::default(),
        }
    }

    /// 设置执行配置
    pub fn with_config(mut self, config: WorkflowConfig) -> Self {
        self.config = config;
        self
    }

    /// 获取源录制
    pub fn recording(&self) -> &BrowserRecording {
        &self.recording
    }

    /// 获取可变录制引用
    pub fn recording_mut(&mut self) -> &mut BrowserRecording {
        &mut self.recording
    }

    /// 验证录制的完整性
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.recording.actions.is_empty() {
            errors.push("Recording has no actions".to_string());
        }

        // 检查参数是否都有对应的占位符
        for param in &self.recording.parameters {
            let placeholder = format!("{{{{{}}}}}", param.name);
            let has_placeholder = self.recording.actions.iter().any(|a| match &a.action {
                BrowserAction::Navigate { url } => url.contains(&placeholder),
                BrowserAction::Input { value, .. } => value.contains(&placeholder),
                BrowserAction::Click { selector } => selector.contains(&placeholder),
                _ => false,
            });

            if !has_placeholder {
                errors.push(format!(
                    "Parameter '{}' defined but no placeholder '{{{{{}}}}}' found in actions",
                    param.name, param.name
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// 将录制导出为可读的 Markdown 格式
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", self.recording.metadata.name));
        md.push_str(&format!("{}\n\n", self.recording.metadata.description));
        md.push_str(&format!(
            "- **Domain**: {}\n",
            self.recording.metadata.domain
        ));
        md.push_str(&format!("- **Steps**: {}\n", self.recording.actions.len()));
        md.push_str(&format!(
            "- **Parameters**: {}\n\n",
            self.recording.parameters.len()
        ));

        md.push_str("## Steps\n\n");
        for action in &self.recording.actions {
            let critical = if action.critical { " ⚠️" } else { "" };
            md.push_str(&format!(
                "{}. {}{}\n",
                action.step + 1,
                action.description,
                critical
            ));
        }

        if !self.recording.parameters.is_empty() {
            md.push_str("\n## Parameters\n\n");
            md.push_str("| Name | Type | Required | Description |\n");
            md.push_str("|------|------|----------|-------------|\n");
            for param in &self.recording.parameters {
                md.push_str(&format!(
                    "| {} | {:?} | {} | {} |\n",
                    param.name, param.param_type, param.required, param.description
                ));
            }
        }

        md
    }
}

#[async_trait]
impl Skill for BrowserWorkflowSkill {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.recording.metadata.description
    }

    fn config(&self) -> SkillConfig {
        SkillGenerator::generate_skill_config(&self.recording)
    }

    async fn execute(&self, params: serde_json::Value) -> KiasResult<serde_json::Value> {
        let start = std::time::Instant::now();

        // 从 params 中提取参数替换映射
        let param_map: HashMap<String, String> = if let Some(obj) = params.as_object() {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        } else {
            HashMap::new()
        };

        // 应用参数替换
        let recording = SkillGenerator::apply_parameters(&self.recording, &param_map);

        // 验证必需参数
        for param_def in &recording.parameters {
            if param_def.required
                && !param_map.contains_key(&param_def.name)
                && param_def.default_value.is_none()
            {
                return Err(KiasError::Validation(format!(
                    "Required parameter '{}' not provided",
                    param_def.name
                )));
            }
        }

        tracing::info!(
            skill = %self.name,
            steps = recording.actions.len(),
            params = param_map.len(),
            "Executing browser workflow skill"
        );

        // 模拟执行工作流
        // 注意：实际浏览器执行需要 Playwright/Puppeteer 集成
        // 这里提供框架和步骤验证逻辑
        let mut step_results = Vec::new();
        let mut extracted_data: HashMap<String, String> = HashMap::new();
        let mut screenshots = Vec::new();
        let mut success = true;
        let mut error = None;
        let mut steps_executed = 0;

        for action in &recording.actions {
            let step_start = std::time::Instant::now();

            // 验证动作参数
            let step_result = match self.validate_and_describe_action(action) {
                Ok(desc) => {
                    tracing::debug!(step = action.step, action = %desc, "Step validated");
                    StepResult {
                        step: action.step,
                        description: desc,
                        success: true,
                        duration_ms: step_start.elapsed().as_millis() as u64,
                        error: None,
                    }
                }
                Err(e) => {
                    let err_msg = format!("Step {} failed: {}", action.step, e);
                    tracing::error!(step = action.step, error = %e, "Step validation failed");

                    if action.critical || self.config.stop_on_assert_failure {
                        success = false;
                        error = Some(err_msg.clone());
                        step_results.push(StepResult {
                            step: action.step,
                            description: action.description.clone(),
                            success: false,
                            duration_ms: step_start.elapsed().as_millis() as u64,
                            error: Some(err_msg),
                        });
                        break;
                    }

                    StepResult {
                        step: action.step,
                        description: action.description.clone(),
                        success: false,
                        duration_ms: step_start.elapsed().as_millis() as u64,
                        error: Some(err_msg),
                    }
                }
            };

            steps_executed += 1;
            step_results.push(step_result);

            // 处理截图动作
            if matches!(action.action, BrowserAction::Screenshot { .. }) {
                screenshots.push(format!("screenshot_step_{}", action.step));
            }

            // 处理文本提取
            if let BrowserAction::ExtractText { output_key, .. } = &action.action {
                extracted_data.insert(
                    output_key.clone(),
                    format!("extracted_value_{}", action.step),
                );
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(serde_json::json!({
            "success": success,
            "steps_executed": steps_executed,
            "total_steps": recording.actions.len(),
            "duration_ms": duration_ms,
            "step_results": step_results,
            "extracted_data": extracted_data,
            "error": error,
            "screenshots": screenshots,
            "skill_name": self.name,
            "recording_id": recording.id,
        }))
    }
}

impl BrowserWorkflowSkill {
    /// 验证并描述一个浏览器动作
    fn validate_and_describe_action(&self, action: &RecordedAction) -> KiasResult<String> {
        match &action.action {
            BrowserAction::Navigate { url } => {
                if url.is_empty() {
                    return Err(KiasError::Validation("Navigate URL is empty".into()));
                }
                Ok(format!("Navigate to {}", url))
            }
            BrowserAction::Click { selector } => {
                if selector.is_empty() {
                    return Err(KiasError::Validation("Click selector is empty".into()));
                }
                Ok(format!("Click '{}'", selector))
            }
            BrowserAction::Input {
                selector, value, ..
            } => {
                if selector.is_empty() {
                    return Err(KiasError::Validation("Input selector is empty".into()));
                }
                Ok(format!("Type '{}' into '{}'", value, selector))
            }
            BrowserAction::WaitForElement {
                selector,
                timeout_ms,
            } => Ok(format!(
                "Wait for '{}' ({}ms timeout)",
                selector, timeout_ms
            )),
            BrowserAction::Wait { duration_ms } => Ok(format!("Wait {}ms", duration_ms)),
            BrowserAction::Screenshot { name } => Ok(format!("Screenshot '{}'", name)),
            BrowserAction::ExtractText {
                selector,
                output_key,
            } => Ok(format!("Extract text from '{}' → {}", selector, output_key)),
            BrowserAction::Submit { selector } => Ok(format!("Submit form '{}'", selector)),
            BrowserAction::KeyPress { key } => Ok(format!("Press key '{}'", key)),
            BrowserAction::Scroll { direction, amount } => {
                Ok(format!("Scroll {:?} {}px", direction, amount))
            }
            BrowserAction::EvaluateJs { script, .. } => {
                let preview = if script.len() > 50 {
                    format!("{}...", &script[..50])
                } else {
                    script.clone()
                };
                Ok(format!("Execute JS: {}", preview))
            }
            BrowserAction::AssertElementExists { selector } => {
                Ok(format!("Assert '{}' exists", selector))
            }
            BrowserAction::AssertTextContains {
                selector,
                expected_text,
            } => Ok(format!(
                "Assert '{}' contains '{}'",
                selector, expected_text
            )),
        }
    }
}

// ── Recording Storage ─────────────────────────────────────────────

/// 录制存储管理器
pub struct RecordingStore {
    recordings: HashMap<String, BrowserRecording>,
}

impl RecordingStore {
    pub fn new() -> Self {
        Self {
            recordings: HashMap::new(),
        }
    }

    /// 保存录制
    pub fn save(&mut self, recording: BrowserRecording) {
        tracing::info!(id = %recording.id, "Saving recording");
        self.recordings.insert(recording.id.clone(), recording);
    }

    /// 获取录制
    pub fn get(&self, id: &str) -> Option<&BrowserRecording> {
        self.recordings.get(id)
    }

    /// 列出所有录制
    pub fn list(&self) -> Vec<&BrowserRecording> {
        self.recordings.values().collect()
    }

    /// 删除录制
    pub fn delete(&mut self, id: &str) -> bool {
        self.recordings.remove(id).is_some()
    }

    /// 按域名搜索
    pub fn find_by_domain(&self, domain: &str) -> Vec<&BrowserRecording> {
        self.recordings
            .values()
            .filter(|r| r.metadata.domain.contains(domain))
            .collect()
    }

    /// 按标签搜索
    pub fn find_by_tag(&self, tag: &str) -> Vec<&BrowserRecording> {
        self.recordings
            .values()
            .filter(|r| r.metadata.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// 录制数量
    pub fn count(&self) -> usize {
        self.recordings.len()
    }
}

impl Default for RecordingStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── BrowserAction Tests ───────────────────────────────────────

    #[test]
    fn test_browser_action_serialization() {
        let action = BrowserAction::Navigate {
            url: "https://example.com".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("Navigate"));
        assert!(json.contains("example.com"));

        let deserialized: BrowserAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_browser_action_click() {
        let action = BrowserAction::Click {
            selector: "#submit-btn".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: BrowserAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_browser_action_input_parameterized() {
        let action = BrowserAction::Input {
            selector: "#email".to_string(),
            value: "{{user_email}}".to_string(),
            parameterized: true,
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("parameterized"));
        let deserialized: BrowserAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_scroll_direction_serialization() {
        let directions = vec![
            ScrollDirection::Up,
            ScrollDirection::Down,
            ScrollDirection::Left,
            ScrollDirection::Right,
        ];
        for dir in directions {
            let json = serde_json::to_string(&dir).unwrap();
            let deserialized: ScrollDirection = serde_json::from_str(&json).unwrap();
            assert_eq!(dir, deserialized);
        }
    }

    // ── WebRecorder Tests ─────────────────────────────────────────

    #[test]
    fn test_recorder_start_and_stop() {
        let mut recorder = WebRecorder::new(RecorderConfig::default());
        let id = recorder.start_recording("Login Flow", "User login sequence", "example.com");
        assert!(!id.is_empty());

        recorder
            .record_action(
                BrowserAction::Navigate {
                    url: "https://example.com/login".to_string(),
                },
                "Go to login page",
                true,
            )
            .unwrap();

        let recording = recorder.stop_recording().unwrap();
        assert_eq!(recording.actions.len(), 1);
        assert_eq!(recording.metadata.name, "Login Flow");
    }

    #[test]
    fn test_recorder_no_active_recording() {
        let mut recorder = WebRecorder::new(RecorderConfig::default());
        let result = recorder.record_action(
            BrowserAction::Navigate {
                url: "https://example.com".to_string(),
            },
            "test",
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_recorder_stop_without_start() {
        let mut recorder = WebRecorder::new(RecorderConfig::default());
        let result = recorder.stop_recording();
        assert!(result.is_err());
    }

    #[test]
    fn test_recorder_max_steps() {
        let config = RecorderConfig {
            max_steps: 2,
            ..Default::default()
        };
        let mut recorder = WebRecorder::new(config);
        recorder.start_recording("test", "test", "test.com");

        recorder
            .record_action(BrowserAction::Wait { duration_ms: 100 }, "step 1", false)
            .unwrap();
        recorder
            .record_action(BrowserAction::Wait { duration_ms: 100 }, "step 2", false)
            .unwrap();

        let result = recorder.record_action(
            BrowserAction::Wait { duration_ms: 100 },
            "step 3 (should fail)",
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_recorder_add_parameter() {
        let mut recorder = WebRecorder::new(RecorderConfig::default());
        recorder.start_recording("test", "test", "test.com");

        let param = ParameterDefinition {
            name: "username".to_string(),
            description: "User's login name".to_string(),
            param_type: ParameterType::String,
            default_value: None,
            required: true,
        };

        recorder.add_parameter(param).unwrap();
        let recording = recorder.stop_recording().unwrap();
        assert_eq!(recording.parameters.len(), 1);
        assert_eq!(recording.parameters[0].name, "username");
    }

    // ── Recording Serialization Tests ─────────────────────────────

    #[test]
    fn test_recording_json_roundtrip() {
        let recording = BrowserRecording {
            id: "test-123".to_string(),
            metadata: RecordingMetadata {
                name: "Test Recording".to_string(),
                description: "A test".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 5000,
                domain: "example.com".to_string(),
                tags: vec!["test".to_string()],
                recorded_by: "tester".to_string(),
            },
            actions: vec![
                RecordedAction {
                    step: 0,
                    action: BrowserAction::Navigate {
                        url: "https://example.com".to_string(),
                    },
                    description: "Go to homepage".to_string(),
                    screenshot: None,
                    duration_ms: 1000,
                    critical: true,
                },
                RecordedAction {
                    step: 1,
                    action: BrowserAction::Click {
                        selector: "#login-btn".to_string(),
                    },
                    description: "Click login".to_string(),
                    screenshot: None,
                    duration_ms: 500,
                    critical: true,
                },
            ],
            parameters: vec![],
            version: "1.0.0".to_string(),
        };

        let json = WebRecorder::to_json(&recording).unwrap();
        let restored = WebRecorder::from_json(&json).unwrap();

        assert_eq!(restored.id, recording.id);
        assert_eq!(restored.actions.len(), 2);
        assert_eq!(restored.metadata.name, "Test Recording");
    }

    // ── SkillGenerator Tests ──────────────────────────────────────

    #[test]
    fn test_skill_generator_basic() {
        let recording = BrowserRecording {
            id: "gen-test".to_string(),
            metadata: RecordingMetadata {
                name: "Search Flow".to_string(),
                description: "Search for something".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 3000,
                domain: "google.com".to_string(),
                tags: vec!["search".to_string()],
                recorded_by: "tester".to_string(),
            },
            actions: vec![
                RecordedAction {
                    step: 0,
                    action: BrowserAction::Navigate {
                        url: "https://google.com".to_string(),
                    },
                    description: "Go to Google".to_string(),
                    screenshot: None,
                    duration_ms: 1000,
                    critical: true,
                },
                RecordedAction {
                    step: 1,
                    action: BrowserAction::Input {
                        selector: "input[name='q']".to_string(),
                        value: "Rust programming".to_string(),
                        parameterized: false,
                    },
                    description: "Type search query".to_string(),
                    screenshot: None,
                    duration_ms: 500,
                    critical: true,
                },
            ],
            parameters: vec![],
            version: "1.0.0".to_string(),
        };

        let skill = SkillGenerator::generate_skill(recording);
        assert_eq!(skill.name(), "web-search-flow");
        assert_eq!(skill.recording().actions.len(), 2);
    }

    #[test]
    fn test_skill_generator_config() {
        let recording = BrowserRecording {
            id: "cfg-test".to_string(),
            metadata: RecordingMetadata {
                name: "Config Test".to_string(),
                description: "Testing config generation".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec!["test".to_string()],
                recorded_by: "tester".to_string(),
            },
            actions: vec![RecordedAction {
                step: 0,
                action: BrowserAction::Navigate {
                    url: "https://test.com".to_string(),
                },
                description: "Navigate".to_string(),
                screenshot: None,
                duration_ms: 0,
                critical: true,
            }],
            parameters: vec![],
            version: "1.0.0".to_string(),
        };

        let config = SkillGenerator::generate_skill_config(&recording);
        assert_eq!(config.name, "web-config-test");
        assert!(config.tags.contains(&"browser".to_string()));
        assert!(config.tags.contains(&"auto-generated".to_string()));
    }

    // ── Parameter Replacement Tests ───────────────────────────────

    #[test]
    fn test_parameter_replacement() {
        let recording = BrowserRecording {
            id: "param-test".to_string(),
            metadata: RecordingMetadata {
                name: "Param Test".to_string(),
                description: "Test".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec![],
                recorded_by: "tester".to_string(),
            },
            actions: vec![
                RecordedAction {
                    step: 0,
                    action: BrowserAction::Navigate {
                        url: "https://{{domain}}/login".to_string(),
                    },
                    description: "Navigate".to_string(),
                    screenshot: None,
                    duration_ms: 0,
                    critical: true,
                },
                RecordedAction {
                    step: 1,
                    action: BrowserAction::Input {
                        selector: "#username".to_string(),
                        value: "{{username}}".to_string(),
                        parameterized: true,
                    },
                    description: "Enter username".to_string(),
                    screenshot: None,
                    duration_ms: 0,
                    critical: true,
                },
            ],
            parameters: vec![],
            version: "1.0.0".to_string(),
        };

        let mut params = HashMap::new();
        params.insert("domain".to_string(), "mysite.com".to_string());
        params.insert("username".to_string(), "alice".to_string());

        let replaced = SkillGenerator::apply_parameters(&recording, &params);

        match &replaced.actions[0].action {
            BrowserAction::Navigate { url } => {
                assert_eq!(url, "https://mysite.com/login");
            }
            _ => panic!("Expected Navigate"),
        }

        match &replaced.actions[1].action {
            BrowserAction::Input { value, .. } => {
                assert_eq!(value, "alice");
            }
            _ => panic!("Expected Input"),
        }
    }

    #[test]
    fn test_replace_placeholders() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Alice".to_string());
        params.insert("age".to_string(), "30".to_string());

        let result = SkillGenerator::replace_placeholders("Hello {{name}}, age {{age}}", &params);
        assert_eq!(result, "Hello Alice, age 30");
    }

    #[test]
    fn test_replace_placeholders_no_match() {
        let params = HashMap::new();
        let result = SkillGenerator::replace_placeholders("no placeholders here", &params);
        assert_eq!(result, "no placeholders here");
    }

    // ── BrowserWorkflowSkill Tests ────────────────────────────────

    #[tokio::test]
    async fn test_workflow_skill_execute_basic() {
        let recording = BrowserRecording {
            id: "exec-test".to_string(),
            metadata: RecordingMetadata {
                name: "Execute Test".to_string(),
                description: "Test execution".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec![],
                recorded_by: "tester".to_string(),
            },
            actions: vec![
                RecordedAction {
                    step: 0,
                    action: BrowserAction::Navigate {
                        url: "https://test.com".to_string(),
                    },
                    description: "Navigate".to_string(),
                    screenshot: None,
                    duration_ms: 0,
                    critical: true,
                },
                RecordedAction {
                    step: 1,
                    action: BrowserAction::Click {
                        selector: "#btn".to_string(),
                    },
                    description: "Click button".to_string(),
                    screenshot: None,
                    duration_ms: 0,
                    critical: false,
                },
            ],
            parameters: vec![],
            version: "1.0.0".to_string(),
        };

        let skill = BrowserWorkflowSkill::new("test-workflow", recording);
        let result = skill.execute(serde_json::json!({})).await.unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["steps_executed"], 2);
        assert_eq!(result["total_steps"], 2);
    }

    #[tokio::test]
    async fn test_workflow_skill_with_parameters() {
        let recording = BrowserRecording {
            id: "param-exec".to_string(),
            metadata: RecordingMetadata {
                name: "Param Exec".to_string(),
                description: "Test with params".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec![],
                recorded_by: "tester".to_string(),
            },
            actions: vec![RecordedAction {
                step: 0,
                action: BrowserAction::Navigate {
                    url: "https://{{domain}}/page".to_string(),
                },
                description: "Navigate".to_string(),
                screenshot: None,
                duration_ms: 0,
                critical: true,
            }],
            parameters: vec![ParameterDefinition {
                name: "domain".to_string(),
                description: "Target domain".to_string(),
                param_type: ParameterType::Url,
                default_value: None,
                required: true,
            }],
            version: "1.0.0".to_string(),
        };

        let skill = BrowserWorkflowSkill::new("param-test", recording);
        let result = skill
            .execute(serde_json::json!({"domain": "mysite.com"}))
            .await
            .unwrap();

        assert_eq!(result["success"], true);
    }

    #[tokio::test]
    async fn test_workflow_skill_missing_required_param() {
        let recording = BrowserRecording {
            id: "missing-param".to_string(),
            metadata: RecordingMetadata {
                name: "Missing Param".to_string(),
                description: "Test missing param".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec![],
                recorded_by: "tester".to_string(),
            },
            actions: vec![RecordedAction {
                step: 0,
                action: BrowserAction::Navigate {
                    url: "https://{{domain}}".to_string(),
                },
                description: "Navigate".to_string(),
                screenshot: None,
                duration_ms: 0,
                critical: true,
            }],
            parameters: vec![ParameterDefinition {
                name: "domain".to_string(),
                description: "Target domain".to_string(),
                param_type: ParameterType::Url,
                default_value: None,
                required: true,
            }],
            version: "1.0.0".to_string(),
        };

        let skill = BrowserWorkflowSkill::new("missing-test", recording);
        let result = skill.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    // ── Validation Tests ──────────────────────────────────────────

    #[test]
    fn test_validate_empty_recording() {
        let recording = BrowserRecording {
            id: "empty".to_string(),
            metadata: RecordingMetadata {
                name: "Empty".to_string(),
                description: "Empty recording".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec![],
                recorded_by: "tester".to_string(),
            },
            actions: vec![],
            parameters: vec![],
            version: "1.0.0".to_string(),
        };

        let skill = BrowserWorkflowSkill::new("empty", recording);
        let errors = skill.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("no actions")));
    }

    #[test]
    fn test_validate_orphan_parameter() {
        let recording = BrowserRecording {
            id: "orphan".to_string(),
            metadata: RecordingMetadata {
                name: "Orphan".to_string(),
                description: "Orphan param test".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec![],
                recorded_by: "tester".to_string(),
            },
            actions: vec![RecordedAction {
                step: 0,
                action: BrowserAction::Navigate {
                    url: "https://test.com".to_string(),
                },
                description: "Navigate".to_string(),
                screenshot: None,
                duration_ms: 0,
                critical: true,
            }],
            parameters: vec![ParameterDefinition {
                name: "unused_param".to_string(),
                description: "Not used anywhere".to_string(),
                param_type: ParameterType::String,
                default_value: None,
                required: false,
            }],
            version: "1.0.0".to_string(),
        };

        let skill = BrowserWorkflowSkill::new("orphan-test", recording);
        let errors = skill.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("unused_param")));
    }

    // ── RecordingStore Tests ──────────────────────────────────────

    #[test]
    fn test_store_save_and_get() {
        let mut store = RecordingStore::new();
        let recording = BrowserRecording {
            id: "store-1".to_string(),
            metadata: RecordingMetadata {
                name: "Store Test".to_string(),
                description: "Test".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec!["test".to_string()],
                recorded_by: "tester".to_string(),
            },
            actions: vec![],
            parameters: vec![],
            version: "1.0.0".to_string(),
        };

        store.save(recording);
        assert_eq!(store.count(), 1);
        assert!(store.get("store-1").is_some());
        assert!(store.get("nonexistent").is_none());
    }

    #[test]
    fn test_store_find_by_domain() {
        let mut store = RecordingStore::new();

        store.save(BrowserRecording {
            id: "d1".to_string(),
            metadata: RecordingMetadata {
                name: "A".to_string(),
                description: "A".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "google.com".to_string(),
                tags: vec![],
                recorded_by: "tester".to_string(),
            },
            actions: vec![],
            parameters: vec![],
            version: "1.0.0".to_string(),
        });

        store.save(BrowserRecording {
            id: "d2".to_string(),
            metadata: RecordingMetadata {
                name: "B".to_string(),
                description: "B".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "github.com".to_string(),
                tags: vec![],
                recorded_by: "tester".to_string(),
            },
            actions: vec![],
            parameters: vec![],
            version: "1.0.0".to_string(),
        });

        let google_results = store.find_by_domain("google");
        assert_eq!(google_results.len(), 1);
    }

    #[test]
    fn test_store_find_by_tag() {
        let mut store = RecordingStore::new();

        store.save(BrowserRecording {
            id: "t1".to_string(),
            metadata: RecordingMetadata {
                name: "A".to_string(),
                description: "A".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec!["login".to_string(), "critical".to_string()],
                recorded_by: "tester".to_string(),
            },
            actions: vec![],
            parameters: vec![],
            version: "1.0.0".to_string(),
        });

        assert_eq!(store.find_by_tag("login").len(), 1);
        assert_eq!(store.find_by_tag("critical").len(), 1);
        assert_eq!(store.find_by_tag("nonexistent").len(), 0);
    }

    #[test]
    fn test_store_delete() {
        let mut store = RecordingStore::new();
        store.save(BrowserRecording {
            id: "del-1".to_string(),
            metadata: RecordingMetadata {
                name: "Del".to_string(),
                description: "Del".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 0,
                domain: "test.com".to_string(),
                tags: vec![],
                recorded_by: "tester".to_string(),
            },
            actions: vec![],
            parameters: vec![],
            version: "1.0.0".to_string(),
        });

        assert_eq!(store.count(), 1);
        assert!(store.delete("del-1"));
        assert_eq!(store.count(), 0);
        assert!(!store.delete("del-1"));
    }

    // ── Markdown Export Tests ─────────────────────────────────────

    #[test]
    fn test_markdown_export() {
        let recording = BrowserRecording {
            id: "md-test".to_string(),
            metadata: RecordingMetadata {
                name: "Login Flow".to_string(),
                description: "User login sequence".to_string(),
                recorded_at: SystemTime::now(),
                duration_ms: 5000,
                domain: "example.com".to_string(),
                tags: vec!["login".to_string()],
                recorded_by: "tester".to_string(),
            },
            actions: vec![
                RecordedAction {
                    step: 0,
                    action: BrowserAction::Navigate {
                        url: "https://example.com/login".to_string(),
                    },
                    description: "Go to login page".to_string(),
                    screenshot: None,
                    duration_ms: 1000,
                    critical: true,
                },
                RecordedAction {
                    step: 1,
                    action: BrowserAction::Click {
                        selector: "#submit".to_string(),
                    },
                    description: "Submit form".to_string(),
                    screenshot: None,
                    duration_ms: 500,
                    critical: false,
                },
            ],
            parameters: vec![ParameterDefinition {
                name: "email".to_string(),
                description: "User email".to_string(),
                param_type: ParameterType::String,
                default_value: None,
                required: true,
            }],
            version: "1.0.0".to_string(),
        };

        let skill = BrowserWorkflowSkill::new("login", recording);
        let md = skill.to_markdown();

        assert!(md.contains("# Login Flow"));
        assert!(md.contains("example.com"));
        assert!(md.contains("1. Go to login page ⚠️"));
        assert!(md.contains("2. Submit form"));
        assert!(md.contains("email"));
    }

    // ── from_json Tests ───────────────────────────────────────────

    #[test]
    fn test_skill_generator_from_json() {
        let json = r#"{
            "id": "json-test",
            "metadata": {
                "name": "JSON Test",
                "description": "From JSON",
                "recorded_at": {"secs_since_epoch": 1735689600, "nanos_since_epoch": 0},
                "duration_ms": 1000,
                "domain": "test.com",
                "tags": [],
                "recorded_by": "tester"
            },
            "actions": [{
                "step": 0,
                "action": {"Navigate": {"url": "https://test.com"}},
                "description": "Go",
                "screenshot": null,
                "duration_ms": 500,
                "critical": true
            }],
            "parameters": [],
            "version": "1.0.0"
        }"#;

        let skill = SkillGenerator::from_json(json).unwrap();
        assert_eq!(skill.name(), "web-json-test");
        assert_eq!(skill.recording().actions.len(), 1);
    }

    // ── WorkflowConfig Tests ──────────────────────────────────────

    #[test]
    fn test_workflow_config_default() {
        let config = WorkflowConfig::default();
        assert_eq!(config.step_timeout_ms, 30_000);
        assert_eq!(config.retry_count, 2);
        assert!(config.stop_on_assert_failure);
    }

    // ── Edge Cases ────────────────────────────────────────────────

    #[test]
    fn test_parameter_type_serialization() {
        let types = vec![
            ParameterType::String,
            ParameterType::Number,
            ParameterType::Boolean,
            ParameterType::Url,
            ParameterType::Selector,
        ];
        for pt in types {
            let json = serde_json::to_string(&pt).unwrap();
            let deserialized: ParameterType = serde_json::from_str(&json).unwrap();
            assert_eq!(pt, deserialized);
        }
    }

    #[test]
    fn test_all_action_types_serialize() {
        let actions = vec![
            BrowserAction::Navigate { url: "u".into() },
            BrowserAction::Click {
                selector: "s".into(),
            },
            BrowserAction::Input {
                selector: "s".into(),
                value: "v".into(),
                parameterized: false,
            },
            BrowserAction::WaitForElement {
                selector: "s".into(),
                timeout_ms: 1000,
            },
            BrowserAction::Wait { duration_ms: 500 },
            BrowserAction::Screenshot { name: "n".into() },
            BrowserAction::ExtractText {
                selector: "s".into(),
                output_key: "k".into(),
            },
            BrowserAction::Submit {
                selector: "s".into(),
            },
            BrowserAction::KeyPress {
                key: "Enter".into(),
            },
            BrowserAction::Scroll {
                direction: ScrollDirection::Down,
                amount: 100,
            },
            BrowserAction::EvaluateJs {
                script: "1+1".into(),
                output_key: None,
            },
            BrowserAction::AssertElementExists {
                selector: "s".into(),
            },
            BrowserAction::AssertTextContains {
                selector: "s".into(),
                expected_text: "t".into(),
            },
        ];

        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let deserialized: BrowserAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, deserialized);
        }
    }
}
