//! 经验积累 — KIAS自循环的闭环（持久化+自适应版）
//!
//! 自动积累修复经验，包括：
//! - 成功/失败模式识别
//! - JSON 文件持久化（重启不丢失）
//! - 自适应可信度调整（贝叶斯更新）
//! - 趋势分析（含修复时间追踪）
//!
//! ## 控制论原理
//! Learner 是闭环的"记忆体"——从历史反馈中学习，调整未来决策。
//! 参考：Wiener Cybernetics — 系统必须存储并利用过去的经验。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// 经验类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LessonType {
    /// 成功经验
    Success,
    /// 失败教训
    Failure,
    /// 优化建议
    Optimization,
    /// 风险警告
    RiskWarning,
    /// 最佳实践
    BestPractice,
}

/// 经验条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonEntry {
    /// 条目ID
    pub id: String,
    /// 经验类型
    pub lesson_type: LessonType,
    /// 标题
    pub title: String,
    /// 内容描述
    pub content: String,
    /// 问题分类
    pub category: String,
    /// 标签
    pub tags: Vec<String>,
    /// 来源循环ID
    pub source_loop_id: Option<String>,
    /// 问题ID
    pub problem_id: Option<String>,
    /// 方案ID
    pub plan_id: Option<String>,
    /// 可信度 (0.0 - 1.0) — 贝叶斯更新
    pub confidence: f64,
    /// 使用次数
    pub usage_count: u32,
    /// 成功次数（用于贝叶斯更新）
    pub success_count: u32,
    /// 失败次数
    pub failure_count: u32,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后使用时间
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 修复结果反馈
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixOutcome {
    /// 关联的经验ID
    pub lesson_id: String,
    /// 是否成功
    pub success: bool,
    /// 修复耗时（秒）
    pub fix_duration_secs: f64,
    /// 根因标签
    pub root_cause_tags: Vec<String>,
    /// 反馈时间
    pub feedback_at: chrono::DateTime<chrono::Utc>,
}

/// 趋势分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    /// 分类
    pub category: String,
    /// 总问题数
    pub total_problems: u32,
    /// 成功修复数
    pub successful_fixes: u32,
    /// 失败修复数
    pub failed_fixes: u32,
    /// 成功率
    pub success_rate: f64,
    /// 平均修复时间（秒）
    pub avg_fix_time_seconds: f64,
    /// 常见根因
    pub common_root_causes: Vec<(String, u32)>,
    /// 推荐方案（基于成功率排序）
    pub recommended_plans: Vec<String>,
}

/// 经验积累器（支持持久化+自适应）
pub struct Learner {
    /// 经验库
    lessons: Vec<LessonEntry>,
    /// 分类索引
    category_index: HashMap<String, Vec<usize>>,
    /// 标签索引
    tag_index: HashMap<String, Vec<usize>>,
    /// 统计数据
    stats: LearnerStats,
    /// 根因频率统计
    root_cause_freq: HashMap<String, u32>,
    /// 修复时间追踪
    fix_times: HashMap<String, Vec<f64>>,
    /// 持久化路径
    persist_path: Option<PathBuf>,
}

/// 学习统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerStats {
    /// 总经验数
    pub total_lessons: u32,
    /// 成功经验数
    pub success_count: u32,
    /// 失败教训数
    pub failure_count: u32,
    /// 平均可信度
    pub avg_confidence: f64,
    /// 最活跃分类
    pub most_active_category: Option<String>,
    /// 总反馈次数
    pub total_feedbacks: u32,
}

impl Default for Learner {
    fn default() -> Self {
        Self::new()
    }
}

/// 持久化数据格式
#[derive(Serialize, Deserialize)]
struct LearnerSnapshot {
    lessons: Vec<LessonEntry>,
    root_cause_freq: HashMap<String, u32>,
    fix_times: HashMap<String, Vec<f64>>,
    total_feedbacks: u32,
}

impl Learner {
    /// 创建新的经验积累器
    pub fn new() -> Self {
        Self {
            lessons: Vec::new(),
            category_index: HashMap::new(),
            tag_index: HashMap::new(),
            stats: LearnerStats {
                total_lessons: 0,
                success_count: 0,
                failure_count: 0,
                avg_confidence: 0.0,
                most_active_category: None,
                total_feedbacks: 0,
            },
            root_cause_freq: HashMap::new(),
            fix_times: HashMap::new(),
            persist_path: None,
        }
    }

    /// 创建带持久化路径的经验积累器
    pub fn with_persistence(path: impl Into<PathBuf>) -> Self {
        let mut learner = Self::new();
        learner.persist_path = Some(path.into());
        // 尝试加载已有数据
        if let Err(e) = learner.load_from_disk() {
            eprintln!("警告：无法加载经验数据: {}", e);
        }
        learner
    }

    /// 记录经验
    pub fn record_lesson(&mut self, mut entry: LessonEntry) -> String {
        let id = entry.id.clone();
        let idx = self.lessons.len();

        // 更新索引
        self.category_index
            .entry(entry.category.clone())
            .or_default()
            .push(idx);

        for tag in &entry.tags {
            self.tag_index.entry(tag.clone()).or_default().push(idx);
        }

        entry.usage_count = 0;
        entry.success_count = 0;
        entry.failure_count = 0;
        entry.last_used_at = None;

        // 更新统计
        match entry.lesson_type {
            LessonType::Success | LessonType::BestPractice => self.stats.success_count += 1,
            LessonType::Failure | LessonType::RiskWarning => self.stats.failure_count += 1,
            _ => {}
        }
        self.stats.total_lessons += 1;

        self.lessons.push(entry);
        self.update_stats();
        self.try_persist();

        id
    }

    /// 反馈修复结果 — 贝叶斯自适应更新
    ///
    /// 核心公式：new_confidence = (prior * prior_weight + observed) / (prior_weight + 1)
    /// 其中 observed = 1.0 if success else 0.0
    pub fn record_outcome(&mut self, outcome: FixOutcome) {
        self.stats.total_feedbacks += 1;

        // 更新根因频率
        for tag in &outcome.root_cause_tags {
            *self.root_cause_freq.entry(tag.clone()).or_insert(0) += 1;
        }

        // 追踪修复时间
        if outcome.fix_duration_secs > 0.0 {
            self.fix_times
                .entry(outcome.lesson_id.clone())
                .or_default()
                .push(outcome.fix_duration_secs);
        }

        // 贝叶斯更新可信度
        if let Some(entry) = self.lessons.iter_mut().find(|e| e.id == outcome.lesson_id) {
            if outcome.success {
                entry.success_count += 1;
            } else {
                entry.failure_count += 1;
            }

            let total = entry.success_count + entry.failure_count;
            let observed = if outcome.success { 1.0 } else { 0.0 };
            // Prior weight: 使用已有经验数作为先验权重
            let prior_weight = (total as f64 * 0.3).max(1.0);
            entry.confidence = (entry.confidence * prior_weight + observed) / (prior_weight + 1.0);

            // 可信度边界
            entry.confidence = entry.confidence.clamp(0.01, 0.99);
        }

        self.try_persist();
    }

    /// 按分类查询经验
    pub fn query_by_category(&self, category: &str) -> Vec<&LessonEntry> {
        self.category_index
            .get(category)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| self.lessons.get(i))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 按标签查询经验
    pub fn query_by_tag(&self, tag: &str) -> Vec<&LessonEntry> {
        self.tag_index
            .get(tag)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| self.lessons.get(i))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 按类型查询经验
    pub fn query_by_type(&self, lesson_type: &LessonType) -> Vec<&LessonEntry> {
        self.lessons
            .iter()
            .filter(|e| &e.lesson_type == lesson_type)
            .collect()
    }

    /// 搜索经验（关键词匹配标题和内容）
    pub fn search(&self, keyword: &str) -> Vec<&LessonEntry> {
        let keyword_lower = keyword.to_lowercase();
        self.lessons
            .iter()
            .filter(|e| {
                e.title.to_lowercase().contains(&keyword_lower)
                    || e.content.to_lowercase().contains(&keyword_lower)
                    || e.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&keyword_lower))
            })
            .collect()
    }

    /// 获取推荐经验（基于可信度和成功率）
    pub fn get_recommendations(&self, category: &str, limit: usize) -> Vec<&LessonEntry> {
        let mut candidates: Vec<&LessonEntry> = self.query_by_category(category);

        // 按可信度 × (1 + 使用对数) 排序
        candidates.sort_by(|a, b| {
            let score_a = a.confidence * (1.0 + (a.usage_count as f64 + 1.0).ln());
            let score_b = b.confidence * (1.0 + (b.usage_count as f64 + 1.0).ln());
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates.into_iter().take(limit).collect()
    }

    /// 记录经验使用
    pub fn mark_used(&mut self, lesson_id: &str) {
        if let Some(entry) = self.lessons.iter_mut().find(|e| e.id == lesson_id) {
            entry.usage_count += 1;
            entry.last_used_at = Some(chrono::Utc::now());
        }
    }

    /// 分析趋势（含修复时间和根因统计）
    pub fn analyze_trends(&self) -> Vec<TrendAnalysis> {
        let mut category_stats: HashMap<String, (u32, u32, u32, Vec<f64>)> = HashMap::new();

        for entry in &self.lessons {
            let stats = category_stats
                .entry(entry.category.clone())
                .or_insert((0, 0, 0, vec![]));
            stats.0 += 1; // total
            match entry.lesson_type {
                LessonType::Success | LessonType::BestPractice => stats.1 += 1,
                LessonType::Failure | LessonType::RiskWarning => stats.2 += 1,
                _ => {}
            }
            // 收集修复时间
            if let Some(times) = self.fix_times.get(&entry.id) {
                stats.3.extend(times);
            }
        }

        // 排序根因
        let mut sorted_causes: Vec<(String, u32)> = self
            .root_cause_freq
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        sorted_causes.sort_by_key(|b| std::cmp::Reverse(b.1));

        category_stats
            .into_iter()
            .map(|(category, (total, successful, failed, times))| {
                let avg_time = if times.is_empty() {
                    0.0
                } else {
                    times.iter().sum::<f64>() / times.len() as f64
                };
                TrendAnalysis {
                    category: category.clone(),
                    total_problems: total,
                    successful_fixes: successful,
                    failed_fixes: failed,
                    success_rate: if total > 0 {
                        successful as f64 / total as f64
                    } else {
                        0.0
                    },
                    avg_fix_time_seconds: avg_time,
                    common_root_causes: sorted_causes.iter().take(5).cloned().collect(),
                    recommended_plans: self
                        .get_recommendations(&category, 3)
                        .iter()
                        .map(|e| e.title.clone())
                        .collect(),
                }
            })
            .collect()
    }

    /// 获取统计
    pub fn stats(&self) -> &LearnerStats {
        &self.stats
    }

    /// 获取所有经验
    pub fn all_lessons(&self) -> &[LessonEntry] {
        &self.lessons
    }

    /// 生成学习报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# KIAS 经验积累报告\n\n");
        report.push_str(&format!(
            "生成时间: {}\n\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));

        report.push_str("## 统计概览\n\n");
        report.push_str(&format!("- 总经验数: {}\n", self.stats.total_lessons));
        report.push_str(&format!("- 成功经验: {}\n", self.stats.success_count));
        report.push_str(&format!("- 失败教训: {}\n", self.stats.failure_count));
        report.push_str(&format!("- 总反馈次数: {}\n", self.stats.total_feedbacks));
        report.push_str(&format!("- 平均可信度: {:.2}\n", self.stats.avg_confidence));
        if let Some(ref cat) = self.stats.most_active_category {
            report.push_str(&format!("- 最活跃分类: {}\n", cat));
        }

        report.push_str("\n## 趋势分析\n\n");
        let trends = self.analyze_trends();
        if trends.is_empty() {
            report.push_str("暂无趋势数据\n");
        } else {
            for trend in &trends {
                report.push_str(&format!("### {}\n", trend.category));
                report.push_str(&format!("- 总问题: {}\n", trend.total_problems));
                report.push_str(&format!("- 成功率: {:.1}%\n", trend.success_rate * 100.0));
                report.push_str(&format!(
                    "- 平均修复时间: {:.1}s\n",
                    trend.avg_fix_time_seconds
                ));
                if !trend.common_root_causes.is_empty() {
                    report.push_str("- 常见根因:\n");
                    for (cause, count) in &trend.common_root_causes {
                        report.push_str(&format!("  - {} ({}次)\n", cause, count));
                    }
                }
                report.push('\n');
            }
        }

        report.push_str("\n## 最近经验\n\n");
        for entry in self.lessons.iter().rev().take(10) {
            report.push_str(&format!(
                "- **[{:?}]** {} (可信度: {:.2}, 成功/失败: {}/{})\n",
                entry.lesson_type,
                entry.title,
                entry.confidence,
                entry.success_count,
                entry.failure_count
            ));
            report.push_str(&format!("  {}\n", entry.content));
        }

        report
    }

    fn update_stats(&mut self) {
        if !self.lessons.is_empty() {
            let total_confidence: f64 = self.lessons.iter().map(|e| e.confidence).sum();
            self.stats.avg_confidence = total_confidence / self.lessons.len() as f64;
        }

        // 找最活跃分类
        let mut max_count = 0;
        for (cat, indices) in &self.category_index {
            if indices.len() > max_count {
                max_count = indices.len();
                self.stats.most_active_category = Some(cat.clone());
            }
        }
    }

    /// 持久化到磁盘
    fn try_persist(&self) {
        if let Some(ref path) = self.persist_path {
            let snapshot = LearnerSnapshot {
                lessons: self.lessons.clone(),
                root_cause_freq: self.root_cause_freq.clone(),
                fix_times: self.fix_times.clone(),
                total_feedbacks: self.stats.total_feedbacks,
            };
            if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// 从磁盘加载
    fn load_from_disk(&mut self) -> Result<(), String> {
        let path = match self.persist_path.as_ref() {
            Some(p) => p,
            None => return Ok(()),
        };

        if !path.exists() {
            return Ok(());
        }

        let data = std::fs::read_to_string(path).map_err(|e| format!("读取失败: {}", e))?;
        let snapshot: LearnerSnapshot =
            serde_json::from_str(&data).map_err(|e| format!("解析失败: {}", e))?;

        // 重建索引
        for (idx, entry) in snapshot.lessons.iter().enumerate() {
            self.category_index
                .entry(entry.category.clone())
                .or_default()
                .push(idx);
            for tag in &entry.tags {
                self.tag_index.entry(tag.clone()).or_default().push(idx);
            }
            match entry.lesson_type {
                LessonType::Success | LessonType::BestPractice => self.stats.success_count += 1,
                LessonType::Failure | LessonType::RiskWarning => self.stats.failure_count += 1,
                _ => {}
            }
        }

        self.stats.total_lessons = snapshot.lessons.len() as u32;
        self.stats.total_feedbacks = snapshot.total_feedbacks;
        self.lessons = snapshot.lessons;
        self.root_cause_freq = snapshot.root_cause_freq;
        self.fix_times = snapshot.fix_times;
        self.update_stats();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(id: &str, lesson_type: LessonType, category: &str) -> LessonEntry {
        LessonEntry {
            id: id.to_string(),
            lesson_type,
            title: format!("Test {}", id),
            content: format!("Content for {}", id),
            category: category.to_string(),
            tags: vec!["test".to_string()],
            source_loop_id: None,
            problem_id: None,
            plan_id: None,
            confidence: 0.8,
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        }
    }

    #[test]
    fn test_learner_creation() {
        let learner = Learner::new();
        assert_eq!(learner.stats().total_lessons, 0);
        assert!(learner.all_lessons().is_empty());
    }

    #[test]
    fn test_record_lesson() {
        let mut learner = Learner::new();
        let entry = create_test_entry("l1", LessonType::Success, "compilation");
        learner.record_lesson(entry);

        assert_eq!(learner.stats().total_lessons, 1);
        assert_eq!(learner.stats().success_count, 1);
    }

    #[test]
    fn test_query_by_category() {
        let mut learner = Learner::new();
        learner.record_lesson(create_test_entry("l1", LessonType::Success, "compilation"));
        learner.record_lesson(create_test_entry("l2", LessonType::Failure, "test"));
        learner.record_lesson(create_test_entry("l3", LessonType::Success, "compilation"));

        let results = learner.query_by_category("compilation");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_by_tag() {
        let mut learner = Learner::new();
        let mut entry = create_test_entry("l1", LessonType::Success, "compilation");
        entry.tags = vec!["rust".to_string(), "build".to_string()];
        learner.record_lesson(entry);

        let results = learner.query_by_tag("rust");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search() {
        let mut learner = Learner::new();
        let mut entry = create_test_entry("l1", LessonType::Success, "compilation");
        entry.title = "修复编译错误".to_string();
        entry.content = "使用cargo build修复".to_string();
        learner.record_lesson(entry);

        let results = learner.search("编译");
        assert_eq!(results.len(), 1);

        let results = learner.search("cargo");
        assert_eq!(results.len(), 1);

        let results = learner.search("不存在");
        assert!(results.is_empty());
    }

    #[test]
    fn test_bayesian_confidence_update() {
        let mut learner = Learner::new();
        learner.record_lesson(create_test_entry("l1", LessonType::Success, "test"));

        let initial_confidence = learner.all_lessons()[0].confidence;

        // 成功反馈 → 可信度应上升
        learner.record_outcome(FixOutcome {
            lesson_id: "l1".to_string(),
            success: true,
            fix_duration_secs: 10.0,
            root_cause_tags: vec!["compilation".to_string()],
            feedback_at: chrono::Utc::now(),
        });

        let updated = learner.all_lessons()[0].confidence;
        assert!(updated >= initial_confidence, "成功反馈应提升可信度");

        // 失败反馈 → 可信度应下降
        learner.record_outcome(FixOutcome {
            lesson_id: "l1".to_string(),
            success: false,
            fix_duration_secs: 30.0,
            root_cause_tags: vec!["logic".to_string()],
            feedback_at: chrono::Utc::now(),
        });

        let after_fail = learner.all_lessons()[0].confidence;
        assert!(after_fail < updated, "失败反馈应降低可信度");
    }

    #[test]
    fn test_trend_analysis_with_fix_times() {
        let mut learner = Learner::new();
        learner.record_lesson(create_test_entry("l1", LessonType::Success, "compilation"));
        learner.record_lesson(create_test_entry("l2", LessonType::Failure, "compilation"));

        // 反馈修复时间
        learner.record_outcome(FixOutcome {
            lesson_id: "l1".to_string(),
            success: true,
            fix_duration_secs: 10.0,
            root_cause_tags: vec!["type_error".to_string()],
            feedback_at: chrono::Utc::now(),
        });

        let trends = learner.analyze_trends();
        let comp = trends.iter().find(|t| t.category == "compilation").unwrap();
        assert!(comp.avg_fix_time_seconds > 0.0);
        assert!(!comp.common_root_causes.is_empty());
    }

    #[test]
    fn test_persistence_roundtrip() {
        let dir = std::env::temp_dir().join("kias_learner_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_learner.json");

        // 写入
        {
            let mut learner = Learner::with_persistence(path.clone());
            learner.record_lesson(create_test_entry("l1", LessonType::Success, "test"));
            learner.record_outcome(FixOutcome {
                lesson_id: "l1".to_string(),
                success: true,
                fix_duration_secs: 5.0,
                root_cause_tags: vec!["build".to_string()],
                feedback_at: chrono::Utc::now(),
            });
        }

        // 读取
        {
            let learner = Learner::with_persistence(path.clone());
            assert_eq!(learner.stats().total_lessons, 1);
            assert_eq!(learner.stats().total_feedbacks, 1);
            assert!(!learner.root_cause_freq.is_empty());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_recommendations_ordering() {
        let mut learner = Learner::new();
        // 低可信度
        let mut low = create_test_entry("low", LessonType::Success, "test");
        low.confidence = 0.2;
        learner.record_lesson(low);
        // 高可信度
        let mut high = create_test_entry("high", LessonType::Success, "test");
        high.confidence = 0.9;
        learner.record_lesson(high);

        let recs = learner.get_recommendations("test", 2);
        assert_eq!(recs[0].id, "high");
    }

    #[test]
    fn test_mark_used() {
        let mut learner = Learner::new();
        learner.record_lesson(create_test_entry("l1", LessonType::Success, "test"));

        learner.mark_used("l1");
        let lessons = learner.all_lessons();
        assert_eq!(lessons[0].usage_count, 1);
        assert!(lessons[0].last_used_at.is_some());
    }

    #[test]
    fn test_generate_report() {
        let mut learner = Learner::new();
        learner.record_lesson(create_test_entry("l1", LessonType::Success, "test"));

        let report = learner.generate_report();
        assert!(report.contains("KIAS 经验积累报告"));
        assert!(report.contains("总经验数: 1"));
    }
}
