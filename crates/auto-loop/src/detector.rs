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

    #[test]
    fn test_data_loss_detector_no_initial_count() {
        let detector = DataLossDetector::new("agents".to_string());
        let result = detector.detect();
        assert!(!result.has_problem);
        assert!(result.problem_type.is_none());
        assert!(result.severity.is_none());
    }

    #[test]
    fn test_data_loss_detector_count_increase() {
        let mut detector = DataLossDetector::new("tasks".to_string());
        detector.update_count(10);
        detector.update_count(20);
        let result = detector.detect();
        assert!(!result.has_problem);
    }

    #[test]
    fn test_data_loss_detector_count_stable() {
        let mut detector = DataLossDetector::new("items".to_string());
        detector.update_count(10);
        detector.update_count(10);
        let result = detector.detect();
        assert!(!result.has_problem);
    }

    #[test]
    fn test_data_loss_detector_from_zero() {
        let mut detector = DataLossDetector::new("data".to_string());
        detector.update_count(0);
        detector.update_count(0);
        let result = detector.detect();
        // 0 -> 0 is not data loss (last > 0 check)
        assert!(!result.has_problem);
    }

    #[test]
    fn test_data_loss_detector_severity_and_data() {
        let mut detector = DataLossDetector::new("agents".to_string());
        detector.update_count(100);
        detector.update_count(50);
        let result = detector.detect();
        assert!(result.has_problem);
        assert_eq!(result.severity, Some(8));
        assert_eq!(result.data.get("data_name").unwrap(), "agents");
        assert_eq!(result.data.get("last_count").unwrap(), "100");
        assert_eq!(result.data.get("current_count").unwrap(), "50");
        assert!(result.description.as_ref().unwrap().contains("100"));
        assert!(result.description.as_ref().unwrap().contains("50"));
    }

    #[test]
    fn test_data_loss_detector_name_and_type() {
        let detector = DataLossDetector::new("test".to_string());
        assert_eq!(detector.name(), "DataLossDetector");
        assert!(matches!(detector.detector_type(), DetectorType::DataLoss));
    }

    #[test]
    fn test_test_failure_detector_all_pass() {
        let mut detector = TestFailureDetector::new();
        detector.add_result(TestResult {
            name: "test1".to_string(),
            passed: true,
            error: None,
            duration_seconds: 0.1,
        });
        detector.add_result(TestResult {
            name: "test2".to_string(),
            passed: true,
            error: None,
            duration_seconds: 0.2,
        });
        let result = detector.detect();
        assert!(!result.has_problem);
        assert!(result.problem_type.is_none());
    }

    #[test]
    fn test_test_failure_detector_empty() {
        let detector = TestFailureDetector::new();
        let result = detector.detect();
        assert!(!result.has_problem);
    }

    #[test]
    fn test_test_failure_detector_multiple_failures() {
        let mut detector = TestFailureDetector::new();
        for i in 0..5 {
            detector.add_result(TestResult {
                name: format!("test_{}", i),
                passed: false,
                error: Some(format!("error_{}", i)),
                duration_seconds: 0.1,
            });
        }
        let result = detector.detect();
        assert!(result.has_problem);
        assert_eq!(result.severity, Some(6));
        assert_eq!(result.data.get("total_tests").unwrap(), "5");
        assert_eq!(result.data.get("failed_tests").unwrap(), "5");
        let desc = result.description.unwrap();
        assert!(desc.contains("5个测试失败"));
    }

    #[test]
    fn test_test_failure_detector_name_and_type() {
        let detector = TestFailureDetector::new();
        assert_eq!(detector.name(), "TestFailureDetector");
        assert!(matches!(
            detector.detector_type(),
            DetectorType::TestFailure
        ));
    }

    #[test]
    fn test_detector_manager_empty() {
        let mut manager = DetectorManager::new();
        let problems = manager.detect_all();
        assert!(problems.is_empty());
        assert!(manager.history().is_empty());
        assert!(manager.recent_problems().is_empty());
    }

    #[test]
    fn test_detector_manager_history_tracking() {
        let mut manager = DetectorManager::new();

        // Add a detector that reports no problem
        let mut det1 = DataLossDetector::new("a".to_string());
        det1.update_count(10);
        manager.register_detector(Box::new(det1));

        // First detect — no problem
        manager.detect_all();
        assert_eq!(manager.history().len(), 1);
        assert!(!manager.history()[0].has_problem);

        // Add a second detector with a problem
        let mut det2 = DataLossDetector::new("b".to_string());
        det2.update_count(100);
        det2.update_count(50);
        manager.register_detector(Box::new(det2));

        manager.detect_all();
        // History: 1 (first detect) + 2 (second detect runs both det1 and det2) = 3
        assert_eq!(manager.history().len(), 3);
        // det1 still has no problem, det2 has a problem
        assert!(!manager.history()[1].has_problem);
        assert!(manager.history()[2].has_problem);
    }

    #[test]
    fn test_detector_manager_recent_problems() {
        let mut manager = DetectorManager::new();

        let mut det = DataLossDetector::new("x".to_string());
        det.update_count(10);
        det.update_count(5);
        manager.register_detector(Box::new(det));

        manager.detect_all();
        let problems = manager.recent_problems();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].has_problem);
    }

    #[test]
    fn test_detector_manager_multiple_detectors() {
        let mut manager = DetectorManager::new();

        // Detector 1: data loss
        let mut det1 = DataLossDetector::new("agents".to_string());
        det1.update_count(100);
        det1.update_count(50);
        manager.register_detector(Box::new(det1));

        // Detector 2: test failure
        let mut det2 = TestFailureDetector::new();
        det2.add_result(TestResult {
            name: "fail".to_string(),
            passed: false,
            error: Some("err".to_string()),
            duration_seconds: 0.1,
        });
        manager.register_detector(Box::new(det2));

        // Detector 3: no problem
        let mut det3 = DataLossDetector::new("tasks".to_string());
        det3.update_count(10);
        det3.update_count(20);
        manager.register_detector(Box::new(det3));

        let problems = manager.detect_all();
        assert_eq!(problems.len(), 2); // det1 and det2 have problems, det3 doesn't
        assert_eq!(manager.history().len(), 3);
    }

    #[test]
    fn test_detection_result_serialization() {
        let mut detector = DataLossDetector::new("test".to_string());
        detector.update_count(10);
        detector.update_count(5);
        let result = detector.detect();

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: DetectionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.has_problem, result.has_problem);
        assert_eq!(deserialized.problem_type, result.problem_type);
        assert_eq!(deserialized.severity, result.severity);
    }

    #[test]
    fn test_test_result_serialization() {
        let result = TestResult {
            name: "test_serial".to_string(),
            passed: false,
            error: Some("boom".to_string()),
            duration_seconds: 1.5,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: TestResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test_serial");
        assert!(!deserialized.passed);
        assert_eq!(deserialized.error, Some("boom".to_string()));
    }

    #[test]
    fn test_detector_type_serialization() {
        let dt = DetectorType::DataLoss;
        let json = serde_json::to_string(&dt).unwrap();
        assert!(json.contains("DataLoss"));

        let dt2 = DetectorType::Performance;
        let json2 = serde_json::to_string(&dt2).unwrap();
        assert!(json2.contains("Performance"));
    }

    #[test]
    fn test_default_trait_implementations() {
        let detector = TestFailureDetector::default();
        assert!(detector.test_results.is_empty());

        let manager = DetectorManager::default();
        assert!(manager.detectors.is_empty());
        assert!(manager.history.is_empty());
    }
}
