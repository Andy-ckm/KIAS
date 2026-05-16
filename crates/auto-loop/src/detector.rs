//! 问题自动检测 — KIAS自循环的核心
//!
//! 自动检测系统问题，包括：
//! - 数据丢失检测
//! - 性能瓶颈检测
//! - 测试失败检测
//! - 配置错误检测

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 检测器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetectorType {
    /// 数据丢失检测
    DataLoss,
    /// 性能瓶颈检测
    Performance,
    /// 测试失败检测
    TestFailure,
    /// 配置错误检测
    ConfigError,
    /// 内存泄漏检测
    MemoryLeak,
    /// 服务不可用检测
    ServiceUnavailable,
}

/// 检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    /// 是否检测到问题
    pub has_problem: bool,
    /// 问题类型
    pub problem_type: Option<String>,
    /// 问题描述
    pub description: Option<String>,
    /// 严重程度 (1-10)
    pub severity: Option<u8>,
    /// 相关数据
    pub data: HashMap<String, String>,
    /// 检测时间
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

/// 检测器 trait
pub trait Detector: Send + Sync {
    /// 执行检测
    fn detect(&self) -> DetectionResult;

    /// 获取检测器名称
    fn name(&self) -> &str;

    /// 获取检测器类型
    fn detector_type(&self) -> DetectorType;
}

/// 数据丢失检测器
pub struct DataLossDetector {
    /// 上次检测时的数据量
    last_count: Option<usize>,
    /// 当前数据量
    current_count: usize,
    /// 数据名称
    data_name: String,
}

impl DataLossDetector {
    pub fn new(data_name: String) -> Self {
        Self {
            last_count: None,
            current_count: 0,
            data_name,
        }
    }

    /// 更新当前数据量
    pub fn update_count(&mut self, count: usize) {
        self.last_count = Some(self.current_count);
        self.current_count = count;
    }
}

impl Detector for DataLossDetector {
    fn detect(&self) -> DetectionResult {
        let has_problem = if let Some(last) = self.last_count {
            // 如果数据量突然减少，可能是数据丢失
            self.current_count < last && last > 0
        } else {
            false
        };

        DetectionResult {
            has_problem,
            problem_type: if has_problem {
                Some("data_loss".to_string())
            } else {
                None
            },
            description: if has_problem {
                Some(format!(
                    "{}数据量从{}减少到{}，可能存在数据丢失",
                    self.data_name,
                    self.last_count.unwrap_or(0),
                    self.current_count
                ))
            } else {
                None
            },
            severity: if has_problem { Some(8) } else { None },
            data: {
                let mut map = HashMap::new();
                map.insert("data_name".to_string(), self.data_name.clone());
                map.insert(
                    "last_count".to_string(),
                    self.last_count.unwrap_or(0).to_string(),
                );
                map.insert("current_count".to_string(), self.current_count.to_string());
                map
            },
            detected_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "DataLossDetector"
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::DataLoss
    }
}

/// 测试失败检测器
pub struct TestFailureDetector {
    /// 测试结果
    test_results: Vec<TestResult>,
}

/// 测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// 测试名称
    pub name: String,
    /// 是否通过
    pub passed: bool,
    /// 错误信息
    pub error: Option<String>,
    /// 运行时间（秒）
    pub duration_seconds: f64,
}

impl Default for TestFailureDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TestFailureDetector {
    pub fn new() -> Self {
        Self {
            test_results: Vec::new(),
        }
    }

    /// 添加测试结果
    pub fn add_result(&mut self, result: TestResult) {
        self.test_results.push(result);
    }
}

impl Detector for TestFailureDetector {
    fn detect(&self) -> DetectionResult {
        let failed_tests: Vec<&TestResult> =
            self.test_results.iter().filter(|r| !r.passed).collect();

        let has_problem = !failed_tests.is_empty();

        DetectionResult {
            has_problem,
            problem_type: if has_problem {
                Some("test_failure".to_string())
            } else {
                None
            },
            description: if has_problem {
                Some(format!(
                    "{}个测试失败: {}",
                    failed_tests.len(),
                    failed_tests
                        .iter()
                        .map(|t| t.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            } else {
                None
            },
            severity: if has_problem { Some(6) } else { None },
            data: {
                let mut map = HashMap::new();
                map.insert(
                    "total_tests".to_string(),
                    self.test_results.len().to_string(),
                );
                map.insert("failed_tests".to_string(), failed_tests.len().to_string());
                map
            },
            detected_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "TestFailureDetector"
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::TestFailure
    }
}

/// 检测器管理器
pub struct DetectorManager {
    /// 检测器列表
    detectors: Vec<Box<dyn Detector>>,
    /// 检测历史
    history: Vec<DetectionResult>,
}

impl Default for DetectorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DetectorManager {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            history: Vec::new(),
        }
    }

    /// 注册检测器
    pub fn register_detector(&mut self, detector: Box<dyn Detector>) {
        self.detectors.push(detector);
    }

    /// 执行所有检测
    pub fn detect_all(&mut self) -> Vec<DetectionResult> {
        let mut results = Vec::new();

        for detector in &self.detectors {
            let result = detector.detect();
            if result.has_problem {
                results.push(result.clone());
            }
            self.history.push(result);
        }

        results
    }

    /// 获取检测历史
    pub fn history(&self) -> &[DetectionResult] {
        &self.history
    }

    /// 获取最近的问题
    pub fn recent_problems(&self) -> Vec<&DetectionResult> {
        self.history.iter().filter(|r| r.has_problem).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_loss_detector() {
        let mut detector = DataLossDetector::new("agents".to_string());
        detector.update_count(10);

        // 没有数据丢失
        let result = detector.detect();
        assert!(!result.has_problem);

        // 模拟数据丢失
        detector.update_count(5);
        let result = detector.detect();
        assert!(result.has_problem);
        assert_eq!(result.problem_type, Some("data_loss".to_string()));
    }

    #[test]
    fn test_test_failure_detector() {
        let mut detector = TestFailureDetector::new();

        // 添加通过的测试
        detector.add_result(TestResult {
            name: "test1".to_string(),
            passed: true,
            error: None,
            duration_seconds: 1.0,
        });

        // 添加失败的测试
        detector.add_result(TestResult {
            name: "test2".to_string(),
            passed: false,
            error: Some("assertion failed".to_string()),
            duration_seconds: 0.5,
        });

        let result = detector.detect();
        assert!(result.has_problem);
        assert_eq!(result.problem_type, Some("test_failure".to_string()));
    }

    #[test]
    fn test_detector_manager() {
        let mut manager = DetectorManager::new();

        let mut detector = DataLossDetector::new("test".to_string());
        detector.update_count(10);
        detector.update_count(5);

        manager.register_detector(Box::new(detector));

        let problems = manager.detect_all();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].has_problem);
    }
}
