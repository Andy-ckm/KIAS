//! 经验积累 — KIAS自循环的闭环
//!
//! 自动积累修复经验，包括：
//! - 成功/失败模式识别
//! - 知识库构建
//! - 智能推荐
//! - 趋势分析

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// 可信度 (0.0 - 1.0)
    pub confidence: f64,
    /// 使用次数
    pub usage_count: u32,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后使用时间
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
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
    pub common_root_causes: Vec<String>,
    /// 推荐方案
    pub recommended_plans: Vec<String>,
}

/// 经验积累器
pub struct Learner {
    /// 经验库
    lessons: Vec<LessonEntry>,
    /// 分类索引
    category_index: HashMap<String, Vec<usize>>,
    /// 标签索引
    tag_index: HashMap<String, Vec<usize>>,
    /// 统计数据
    stats: LearnerStats,
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
}

impl Default for Learner {
    fn default() -> Self {
        Self::new()
    }
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
            },
        }
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

        id
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

    /// 获取推荐经验（基于可信度和使用次数）
    pub fn get_recommendations(&self, category: &str, limit: usize) -> Vec<&LessonEntry> {
        let mut candidates: Vec<&LessonEntry> = self.query_by_category(category);

        // 按可信度 * 使用次数排序
        candidates.sort_by(|a, b| {
            let score_a = a.confidence * (1.0 + a.usage_count as f64 * 0.1);
            let score_b = b.confidence * (1.0 + b.usage_count as f64 * 0.1);
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

    /// 分析趋势
    pub fn analyze_trends(&self) -> Vec<TrendAnalysis> {
        let mut category_stats: HashMap<String, (u32, u32, u32)> = HashMap::new();

        for entry in &self.lessons {
            let stats = category_stats
                .entry(entry.category.clone())
                .or_insert((0, 0, 0));
            stats.0 += 1; // total
            match entry.lesson_type {
                LessonType::Success | LessonType::BestPractice => stats.1 += 1, // successful
                LessonType::Failure | LessonType::RiskWarning => stats.2 += 1,  // failed
                _ => {}
            }
        }

        category_stats
            .into_iter()
            .map(|(category, (total, successful, failed))| TrendAnalysis {
                category,
                total_problems: total,
                successful_fixes: successful,
                failed_fixes: failed,
                success_rate: if total > 0 {
                    successful as f64 / total as f64
                } else {
                    0.0
                },
                avg_fix_time_seconds: 0.0, // 简化实现
                common_root_causes: vec![],
                recommended_plans: vec![],
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
                report.push('\n');
            }
        }

        report.push_str("\n## 最近经验\n\n");
        for entry in self.lessons.iter().rev().take(10) {
            report.push_str(&format!(
                "- **[{:?}]** {} (可信度: {:.2})\n",
                entry.lesson_type, entry.title, entry.confidence
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
    fn test_recommendations() {
        let mut learner = Learner::new();
        for i in 0..10 {
            let mut entry =
                create_test_entry(&format!("l{}", i), LessonType::Success, "compilation");
            entry.confidence = 0.5 + (i as f64 * 0.05);
            learner.record_lesson(entry);
        }

        let recs = learner.get_recommendations("compilation", 3);
        assert_eq!(recs.len(), 3);
        // 应该按可信度排序
        assert!(recs[0].confidence >= recs[1].confidence);
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
    fn test_trend_analysis() {
        let mut learner = Learner::new();
        learner.record_lesson(create_test_entry("l1", LessonType::Success, "compilation"));
        learner.record_lesson(create_test_entry("l2", LessonType::Success, "compilation"));
        learner.record_lesson(create_test_entry("l3", LessonType::Failure, "compilation"));
        learner.record_lesson(create_test_entry("l4", LessonType::Success, "test"));

        let trends = learner.analyze_trends();
        assert!(!trends.is_empty());

        let compilation_trend = trends.iter().find(|t| t.category == "compilation").unwrap();
        assert_eq!(compilation_trend.total_problems, 3);
        assert_eq!(compilation_trend.successful_fixes, 2);
        assert_eq!(compilation_trend.failed_fixes, 1);
        assert!((compilation_trend.success_rate - 0.666).abs() < 0.01);
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
