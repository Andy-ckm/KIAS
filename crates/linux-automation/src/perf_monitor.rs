//! R030: Linux 性能监控和优化模块
//!
//! 核心能力:
//! - 性能指标采集（CPU/内存/磁盘IO/网络/负载/Swap）
//! - 基线建立（从历史数据计算正常范围）
//! - 异常检测（基于标准差的统计异常检测）
//! - 瓶颈分析（多维度评分，定位主要瓶颈）
//! - 优化建议（基于瓶颈类型生成调优建议）
//!
//! 灵魂: 可追溯(审计日志) / 透明(实时报告) / 可控(阈值可配)
//! 参考: sysstat(sar/iostat/mpstat), bottom(Rust系统监控), intel/pcm

use chrono::Utc;
use tracing::{info, warn};

use crate::error::{AutomationError, Result};
use crate::executor::TaskExecutor;
use crate::models::*;

/// 性能监控引擎
pub struct PerfMonitor {
    config: PerfMonitorConfig,
}

impl PerfMonitor {
    /// 创建性能监控器
    pub fn new(config: PerfMonitorConfig) -> Self {
        Self { config }
    }

    /// 使用默认配置创建
    pub fn with_defaults() -> Self {
        Self::new(PerfMonitorConfig::default())
    }

    /// 采集单个主机的性能指标
    pub async fn collect_metrics(
        &self,
        executor: &TaskExecutor,
        host: &str,
    ) -> Result<Vec<PerfSample>> {
        let mut samples = Vec::new();
        let now = Utc::now();

        for metric_type in &self.config.metrics {
            let (value, unit) = self
                .collect_single_metric(executor, host, metric_type)
                .await?;
            samples.push(PerfSample {
                metric_type: metric_type.clone(),
                value,
                unit,
                timestamp: now,
                host: host.to_string(),
            });
        }

        info!(
            host = %host,
            metrics_count = samples.len(),
            "性能指标采集完成"
        );

        Ok(samples)
    }

    /// 采集单个指标
    async fn collect_single_metric(
        &self,
        executor: &TaskExecutor,
        host: &str,
        metric_type: &PerfMetricType,
    ) -> Result<(f64, String)> {
        let cmd = match metric_type {
            PerfMetricType::CpuUsage => "top -bn1 | grep 'Cpu(s)' | awk '{print $2}'",
            PerfMetricType::MemoryUsage => "free | awk '/Mem:/ {printf \"%.1f\", $3/$2*100}'",
            PerfMetricType::DiskIoRead => "iostat -d 1 2 | tail -1 | awk '{print $3}'",
            PerfMetricType::DiskIoWrite => "iostat -d 1 2 | tail -1 | awk '{print $4}'",
            PerfMetricType::NetworkRx => {
                "cat /proc/net/dev | awk 'NR>2{rx+=$2}END{print rx/1024/1024}'"
            }
            PerfMetricType::NetworkTx => {
                "cat /proc/net/dev | awk 'NR>2{tx+=$10}END{print tx/1024/1024}'"
            }
            PerfMetricType::LoadAverage => "cat /proc/loadavg | awk '{print $1}'",
            PerfMetricType::SwapUsage => {
                "free | awk '/Swap:/ {if($2>0) printf \"%.1f\", $3/$2*100; else print 0}'"
            }
            PerfMetricType::DiskUsage => "df -h / | awk 'NR==2{print $5}' | tr -d '%'",
            PerfMetricType::ProcessCount => "ps aux | wc -l",
            PerfMetricType::ContextSwitches => "vmstat 1 2 | tail -1 | awk '{print $12}'",
            PerfMetricType::Interrupts => "vmstat 1 2 | tail -1 | awk '{print $11}'",
        };

        let result = executor.execute_command(&[host.to_string()], cmd).await?;

        let output = result
            .host_results
            .first()
            .map(|r| r.stdout.trim().to_string())
            .unwrap_or_default();

        let value = output.parse::<f64>().map_err(|e| {
            AutomationError::PerfMonitor(format!("解析指标失败: {} - '{}'", e, output))
        })?;

        let unit = match metric_type {
            PerfMetricType::CpuUsage
            | PerfMetricType::MemoryUsage
            | PerfMetricType::SwapUsage
            | PerfMetricType::DiskUsage => "%",
            PerfMetricType::DiskIoRead | PerfMetricType::DiskIoWrite => "KB/s",
            PerfMetricType::NetworkRx | PerfMetricType::NetworkTx => "MB",
            PerfMetricType::LoadAverage => "",
            PerfMetricType::ProcessCount
            | PerfMetricType::ContextSwitches
            | PerfMetricType::Interrupts => "count",
        };

        Ok((value, unit.to_string()))
    }

    /// 从历史样本建立基线
    pub fn establish_baseline(
        &self,
        samples: &[PerfSample],
        metric_type: &PerfMetricType,
    ) -> Result<PerfBaseline> {
        let values: Vec<f64> = samples
            .iter()
            .filter(|s| s.metric_type == *metric_type)
            .map(|s| s.value)
            .collect();

        if values.len() < self.config.min_baseline_samples as usize {
            return Err(AutomationError::BaselineInsufficient(format!(
                "{:?} 需要 {} 个样本, 当前只有 {} 个",
                metric_type,
                self.config.min_baseline_samples,
                values.len()
            )));
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        let mut sorted = values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        let p99_idx = (sorted.len() as f64 * 0.99) as usize;
        let p95 = sorted[p95_idx.min(sorted.len() - 1)];
        let p99 = sorted[p99_idx.min(sorted.len() - 1)];

        Ok(PerfBaseline {
            metric_type: metric_type.clone(),
            mean,
            std_dev,
            min,
            max,
            p95,
            p99,
            sample_count: values.len() as u32,
            established_at: Utc::now(),
        })
    }

    /// 检测异常（基于标准差）
    pub fn detect_anomalies(
        &self,
        samples: &[PerfSample],
        baselines: &[PerfBaseline],
    ) -> Vec<PerfAnomaly> {
        let mut anomalies = Vec::new();

        for sample in samples {
            if let Some(baseline) = baselines
                .iter()
                .find(|b| b.metric_type == sample.metric_type)
            {
                if baseline.std_dev == 0.0 {
                    continue;
                }

                let deviation = (sample.value - baseline.mean).abs() / baseline.std_dev;

                if deviation >= self.config.critical_sigma_threshold {
                    anomalies.push(PerfAnomaly {
                        metric_type: sample.metric_type.clone(),
                        value: sample.value,
                        baseline_mean: baseline.mean,
                        deviation_sigma: deviation,
                        severity: AnomalySeverity::Critical,
                        message: format!(
                            "{:?} 值 {} 严重偏离基线均值 {} ({:.1}σ)",
                            sample.metric_type, sample.value, baseline.mean, deviation
                        ),
                        detected_at: Utc::now(),
                    });
                } else if deviation >= self.config.anomaly_sigma_threshold {
                    anomalies.push(PerfAnomaly {
                        metric_type: sample.metric_type.clone(),
                        value: sample.value,
                        baseline_mean: baseline.mean,
                        deviation_sigma: deviation,
                        severity: AnomalySeverity::Warning,
                        message: format!(
                            "{:?} 值 {} 偏离基线均值 {} ({:.1}σ)",
                            sample.metric_type, sample.value, baseline.mean, deviation
                        ),
                        detected_at: Utc::now(),
                    });
                }
            }
        }

        if !anomalies.is_empty() {
            warn!(count = anomalies.len(), "检测到性能异常");
        }

        anomalies
    }

    /// 瓶颈分析
    pub fn analyze_bottleneck(&self, samples: &[PerfSample]) -> BottleneckAnalysis {
        let get_value = |metric_type: &PerfMetricType| -> f64 {
            samples
                .iter()
                .find(|s| s.metric_type == *metric_type)
                .map(|s| s.value)
                .unwrap_or(0.0)
        };

        let cpu = get_value(&PerfMetricType::CpuUsage);
        let mem = get_value(&PerfMetricType::MemoryUsage);
        let disk_read = get_value(&PerfMetricType::DiskIoRead);
        let disk_write = get_value(&PerfMetricType::DiskIoWrite);
        let swap = get_value(&PerfMetricType::SwapUsage);
        let load = get_value(&PerfMetricType::LoadAverage);
        let procs = get_value(&PerfMetricType::ProcessCount);

        // 评分: 0-100, 越高越瓶颈
        let cpu_score = cpu; // CPU使用率直接作为分数
        let memory_score = mem;
        let disk_io_score = (disk_read + disk_write).min(100.0);
        let network_score = {
            let rx = get_value(&PerfMetricType::NetworkRx);
            let tx = get_value(&PerfMetricType::NetworkTx);
            ((rx + tx) / 10.0).min(100.0) // 归一化
        };

        // Swap抖动检测
        let swap_score = if swap > 50.0 { swap } else { 0.0 };

        // 进程饱和检测
        let proc_score = if procs > 500.0 {
            (procs / 10.0).min(100.0)
        } else {
            0.0
        };

        // 确定主要和次要瓶颈
        let scores = vec![
            (BottleneckType::CpuBound, cpu_score),
            (BottleneckType::MemoryBound, memory_score),
            (BottleneckType::DiskIoBound, disk_io_score),
            (BottleneckType::NetworkBound, network_score),
            (BottleneckType::SwapThrashing, swap_score),
            (BottleneckType::ProcessSaturation, proc_score),
        ];

        let mut sorted_scores = scores.clone();
        sorted_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let primary = if sorted_scores[0].1 > 70.0 {
            sorted_scores[0].0.clone()
        } else {
            BottleneckType::NoBottleneck
        };

        let secondary = if sorted_scores.len() > 1 && sorted_scores[1].1 > 70.0 {
            Some(sorted_scores[1].0.clone())
        } else {
            None
        };

        let description = match &primary {
            BottleneckType::CpuBound => format!("CPU瓶颈: 使用率{:.1}%, 负载{:.2}", cpu, load),
            BottleneckType::MemoryBound => format!("内存瓶颈: 使用率{:.1}%, Swap{:.1}%", mem, swap),
            BottleneckType::DiskIoBound => format!(
                "磁盘IO瓶颈: 读{:.1}KB/s, 写{:.1}KB/s",
                disk_read, disk_write
            ),
            BottleneckType::NetworkBound => format!(
                "网络瓶颈: 接收{:.1}MB, 发送{:.1}MB",
                get_value(&PerfMetricType::NetworkRx),
                get_value(&PerfMetricType::NetworkTx)
            ),
            BottleneckType::SwapThrashing => format!("Swap抖动: 使用率{:.1}%", swap),
            BottleneckType::ProcessSaturation => format!("进程饱和: {}个进程", procs),
            BottleneckType::NoBottleneck => "系统运行正常,无明显瓶颈".to_string(),
        };

        BottleneckAnalysis {
            primary,
            secondary,
            cpu_score,
            memory_score,
            disk_io_score,
            network_score,
            description,
        }
    }

    /// 生成优化建议
    pub fn generate_recommendations(
        &self,
        bottleneck: &BottleneckAnalysis,
        samples: &[PerfSample],
    ) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();

        match &bottleneck.primary {
            BottleneckType::CpuBound => {
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::CpuBound,
                    priority: 1,
                    title: "识别CPU密集型进程".to_string(),
                    description: "使用top/htop识别CPU使用最高的进程,检查是否有异常进程".to_string(),
                    expected_improvement: "定位并优化高CPU进程可降低CPU使用率20-50%".to_string(),
                    command: Some("top -bn1 -o %CPU | head -20".to_string()),
                });
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::CpuBound,
                    priority: 2,
                    title: "检查CPU频率调节策略".to_string(),
                    description: "确认CPU governor是否为performance模式".to_string(),
                    expected_improvement: "切换到performance模式可提升计算密集型任务性能"
                        .to_string(),
                    command: Some(
                        "cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor".to_string(),
                    ),
                });
            }
            BottleneckType::MemoryBound => {
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::MemoryBound,
                    priority: 1,
                    title: "识别内存消耗大户".to_string(),
                    description: "检查哪些进程占用内存最多,是否有内存泄漏".to_string(),
                    expected_improvement: "释放不必要的内存可降低使用率10-30%".to_string(),
                    command: Some("ps aux --sort=-%mem | head -20".to_string()),
                });
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::MemoryBound,
                    priority: 2,
                    title: "清理缓存和Buffer".to_string(),
                    description: "如果内存压力大,可考虑清理页面缓存".to_string(),
                    expected_improvement: "清理缓存可释放数GB内存".to_string(),
                    command: Some("sync && echo 3 > /proc/sys/vm/drop_caches".to_string()),
                });
            }
            BottleneckType::DiskIoBound => {
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::DiskIoBound,
                    priority: 1,
                    title: "识别IO密集型进程".to_string(),
                    description: "使用iotop找出哪些进程产生大量磁盘IO".to_string(),
                    expected_improvement: "优化IO密集进程可降低磁盘等待50%+".to_string(),
                    command: Some("iotop -bon1 | head -20".to_string()),
                });
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::DiskIoBound,
                    priority: 2,
                    title: "检查磁盘健康状态".to_string(),
                    description: "使用SMART检查磁盘是否有硬件问题".to_string(),
                    expected_improvement: "排除硬件故障可避免IO性能持续下降".to_string(),
                    command: Some("smartctl -a /dev/sda".to_string()),
                });
            }
            BottleneckType::SwapThrashing => {
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::SwapThrashing,
                    priority: 1,
                    title: "降低Swap使用".to_string(),
                    description: "Swap使用率过高会导致系统严重变慢,需要释放内存或增加物理内存"
                        .to_string(),
                    expected_improvement: "降低Swap使用可显著提升系统响应速度".to_string(),
                    command: Some("swapon -s && free -h".to_string()),
                });
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::SwapThrashing,
                    priority: 2,
                    title: "调整swappiness".to_string(),
                    description: "降低vm.swappiness可减少系统使用Swap的倾向".to_string(),
                    expected_improvement: "降低swappiness可减少不必要的Swap使用".to_string(),
                    command: Some("sysctl vm.swappiness".to_string()),
                });
            }
            BottleneckType::ProcessSaturation => {
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::ProcessSaturation,
                    priority: 1,
                    title: "检查僵尸进程".to_string(),
                    description: "僵尸进程会占用PID资源,需要清理父进程".to_string(),
                    expected_improvement: "清理僵尸进程可释放PID和进程表资源".to_string(),
                    command: Some("ps aux | awk '$8==\"Z\"'".to_string()),
                });
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::ProcessSaturation,
                    priority: 2,
                    title: "检查进程数限制".to_string(),
                    description: "确认系统和用户的进程数限制是否合理".to_string(),
                    expected_improvement: "调整ulimit可避免进程创建失败".to_string(),
                    command: Some("ulimit -u && cat /proc/sys/kernel/pid_max".to_string()),
                });
            }
            BottleneckType::NetworkBound => {
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::NetworkBound,
                    priority: 1,
                    title: "检查网络连接状态".to_string(),
                    description: "查看网络连接数和状态,排查异常连接".to_string(),
                    expected_improvement: "清理异常连接可释放网络资源".to_string(),
                    command: Some("ss -s".to_string()),
                });
            }
            BottleneckType::NoBottleneck => {
                // 无瓶颈时仍提供基线优化建议
                recommendations.push(OptimizationRecommendation {
                    category: BottleneckType::NoBottleneck,
                    priority: 3,
                    title: "持续监控".to_string(),
                    description: "系统当前运行正常,建议持续监控以建立性能基线".to_string(),
                    expected_improvement: "及早发现性能退化趋势".to_string(),
                    command: None,
                });
            }
        }

        // 通用建议: 如果负载高于CPU核心数
        let load = samples
            .iter()
            .find(|s| s.metric_type == PerfMetricType::LoadAverage)
            .map(|s| s.value)
            .unwrap_or(0.0);
        if load > 4.0 {
            recommendations.push(OptimizationRecommendation {
                category: BottleneckType::CpuBound,
                priority: 2,
                title: "负载过高".to_string(),
                description: format!("负载 {:.2} 超过合理范围,检查是否有进程竞争CPU", load),
                expected_improvement: "降低负载可提升整体系统响应".to_string(),
                command: Some("uptime && mpstat 1 1".to_string()),
            });
        }

        recommendations.sort_by_key(|r| r.priority);
        recommendations
    }

    /// 生成完整性能报告
    pub async fn generate_report(
        &self,
        executor: &TaskExecutor,
        host: &str,
        baselines: &[PerfBaseline],
    ) -> Result<PerfReport> {
        let samples = self.collect_metrics(executor, host).await?;
        let anomalies = self.detect_anomalies(&samples, baselines);
        let bottleneck = self.analyze_bottleneck(&samples);
        let recommendations = self.generate_recommendations(&bottleneck, &samples);

        // 计算总分: 0-100, 越高越好
        let overall_score = match &bottleneck.primary {
            BottleneckType::NoBottleneck => 90.0,
            BottleneckType::CpuBound => 100.0 - bottleneck.cpu_score,
            BottleneckType::MemoryBound => 100.0 - bottleneck.memory_score,
            BottleneckType::DiskIoBound => 100.0 - bottleneck.disk_io_score,
            BottleneckType::NetworkBound => 100.0 - bottleneck.network_score,
            BottleneckType::SwapThrashing => 30.0,
            BottleneckType::ProcessSaturation => 40.0,
        };

        info!(
            host = %host,
            score = overall_score,
            anomalies = anomalies.len(),
            bottleneck = ?bottleneck.primary,
            "性能报告生成完成"
        );

        Ok(PerfReport {
            host: host.to_string(),
            collected_at: Utc::now(),
            samples,
            baselines: baselines.to_vec(),
            anomalies,
            bottleneck,
            recommendations,
            overall_score,
        })
    }

    /// 获取配置
    pub fn config(&self) -> &PerfMonitorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(metric_type: PerfMetricType, value: f64) -> PerfSample {
        PerfSample {
            metric_type,
            value,
            unit: "%".to_string(),
            timestamp: Utc::now(),
            host: "test-host".to_string(),
        }
    }

    fn baseline(metric_type: PerfMetricType, mean: f64, std_dev: f64) -> PerfBaseline {
        PerfBaseline {
            metric_type,
            mean,
            std_dev,
            min: mean - 3.0 * std_dev,
            max: mean + 3.0 * std_dev,
            p95: mean + 1.65 * std_dev,
            p99: mean + 2.33 * std_dev,
            sample_count: 100,
            established_at: Utc::now(),
        }
    }

    #[test]
    fn test_perf_monitor_default_config() {
        let monitor = PerfMonitor::with_defaults();
        assert_eq!(monitor.config().interval_secs, 60);
        assert_eq!(monitor.config().min_baseline_samples, 30);
        assert_eq!(monitor.config().anomaly_sigma_threshold, 2.0);
        assert_eq!(monitor.config().critical_sigma_threshold, 3.0);
        assert_eq!(monitor.config().metrics.len(), 8);
    }

    #[test]
    fn test_perf_monitor_custom_config() {
        let config = PerfMonitorConfig {
            interval_secs: 30,
            min_baseline_samples: 10,
            anomaly_sigma_threshold: 1.5,
            critical_sigma_threshold: 2.5,
            metrics: vec![PerfMetricType::CpuUsage, PerfMetricType::MemoryUsage],
        };
        let monitor = PerfMonitor::new(config);
        assert_eq!(monitor.config().interval_secs, 30);
        assert_eq!(monitor.config().metrics.len(), 2);
    }

    #[test]
    fn test_establish_baseline_success() {
        let monitor = PerfMonitor::with_defaults();
        let samples: Vec<PerfSample> = (0..50)
            .map(|i| sample(PerfMetricType::CpuUsage, 50.0 + (i as f64 % 10.0)))
            .collect();

        let result = monitor.establish_baseline(&samples, &PerfMetricType::CpuUsage);
        assert!(result.is_ok());

        let bl = result.unwrap();
        assert_eq!(bl.sample_count, 50);
        assert!(bl.mean > 49.0 && bl.mean < 56.0);
        assert!(bl.std_dev > 0.0);
        assert!(bl.p95 >= bl.mean);
        assert!(bl.p99 >= bl.p95);
    }

    #[test]
    fn test_establish_baseline_insufficient_samples() {
        let monitor = PerfMonitor::with_defaults();
        let samples: Vec<PerfSample> = (0..5)
            .map(|i| sample(PerfMetricType::CpuUsage, 50.0 + i as f64))
            .collect();

        let result = monitor.establish_baseline(&samples, &PerfMetricType::CpuUsage);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AutomationError::BaselineInsufficient(_)
        ));
    }

    #[test]
    fn test_establish_baseline_custom_threshold() {
        let config = PerfMonitorConfig {
            min_baseline_samples: 5,
            ..Default::default()
        };
        let monitor = PerfMonitor::new(config);
        let samples: Vec<PerfSample> = (0..10)
            .map(|i| sample(PerfMetricType::CpuUsage, 50.0 + i as f64))
            .collect();

        let result = monitor.establish_baseline(&samples, &PerfMetricType::CpuUsage);
        assert!(result.is_ok());
    }

    #[test]
    fn test_detect_anomalies_no_anomaly() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![sample(PerfMetricType::CpuUsage, 50.0)];
        let baselines = vec![baseline(PerfMetricType::CpuUsage, 50.0, 5.0)];

        let anomalies = monitor.detect_anomalies(&samples, &baselines);
        assert!(anomalies.is_empty());
    }

    #[test]
    fn test_detect_anomalies_warning() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![sample(PerfMetricType::CpuUsage, 62.0)]; // 2.4σ from 50
        let baselines = vec![baseline(PerfMetricType::CpuUsage, 50.0, 5.0)];

        let anomalies = monitor.detect_anomalies(&samples, &baselines);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].severity, AnomalySeverity::Warning);
        assert!(anomalies[0].deviation_sigma >= 2.0);
    }

    #[test]
    fn test_detect_anomalies_critical() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![sample(PerfMetricType::CpuUsage, 70.0)]; // 4σ from 50
        let baselines = vec![baseline(PerfMetricType::CpuUsage, 50.0, 5.0)];

        let anomalies = monitor.detect_anomalies(&samples, &baselines);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].severity, AnomalySeverity::Critical);
        assert!(anomalies[0].deviation_sigma >= 3.0);
    }

    #[test]
    fn test_detect_anomalies_multiple_metrics() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![
            sample(PerfMetricType::CpuUsage, 50.0),    // normal
            sample(PerfMetricType::MemoryUsage, 95.0), // critical
        ];
        let baselines = vec![
            baseline(PerfMetricType::CpuUsage, 50.0, 5.0),
            baseline(PerfMetricType::MemoryUsage, 50.0, 10.0),
        ];

        let anomalies = monitor.detect_anomalies(&samples, &baselines);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].metric_type, PerfMetricType::MemoryUsage);
    }

    #[test]
    fn test_detect_anomalies_no_baseline() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![sample(PerfMetricType::CpuUsage, 99.0)];
        let baselines = vec![]; // no baseline

        let anomalies = monitor.detect_anomalies(&samples, &baselines);
        assert!(anomalies.is_empty()); // can't detect without baseline
    }

    #[test]
    fn test_detect_anomalies_zero_std_dev() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![sample(PerfMetricType::CpuUsage, 50.0)];
        let baselines = vec![baseline(PerfMetricType::CpuUsage, 50.0, 0.0)];

        let anomalies = monitor.detect_anomalies(&samples, &baselines);
        assert!(anomalies.is_empty()); // zero std_dev means no detection
    }

    #[test]
    fn test_analyze_bottleneck_no_bottleneck() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![
            sample(PerfMetricType::CpuUsage, 30.0),
            sample(PerfMetricType::MemoryUsage, 40.0),
            sample(PerfMetricType::DiskIoRead, 10.0),
            sample(PerfMetricType::DiskIoWrite, 10.0),
            sample(PerfMetricType::NetworkRx, 5.0),
            sample(PerfMetricType::NetworkTx, 5.0),
            sample(PerfMetricType::SwapUsage, 0.0),
            sample(PerfMetricType::LoadAverage, 1.0),
            sample(PerfMetricType::ProcessCount, 100.0),
        ];

        let bottleneck = monitor.analyze_bottleneck(&samples);
        assert_eq!(bottleneck.primary, BottleneckType::NoBottleneck);
        assert!(bottleneck.secondary.is_none());
    }

    #[test]
    fn test_analyze_bottleneck_cpu_bound() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![
            sample(PerfMetricType::CpuUsage, 95.0),
            sample(PerfMetricType::MemoryUsage, 30.0),
            sample(PerfMetricType::DiskIoRead, 10.0),
            sample(PerfMetricType::DiskIoWrite, 10.0),
            sample(PerfMetricType::SwapUsage, 0.0),
            sample(PerfMetricType::LoadAverage, 8.0),
            sample(PerfMetricType::ProcessCount, 100.0),
        ];

        let bottleneck = monitor.analyze_bottleneck(&samples);
        assert_eq!(bottleneck.primary, BottleneckType::CpuBound);
        assert!(bottleneck.description.contains("CPU瓶颈"));
    }

    #[test]
    fn test_analyze_bottleneck_memory_bound() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![
            sample(PerfMetricType::CpuUsage, 30.0),
            sample(PerfMetricType::MemoryUsage, 92.0),
            sample(PerfMetricType::DiskIoRead, 10.0),
            sample(PerfMetricType::DiskIoWrite, 10.0),
            sample(PerfMetricType::SwapUsage, 60.0),
            sample(PerfMetricType::LoadAverage, 2.0),
            sample(PerfMetricType::ProcessCount, 100.0),
        ];

        let bottleneck = monitor.analyze_bottleneck(&samples);
        assert_eq!(bottleneck.primary, BottleneckType::MemoryBound);
        assert!(bottleneck.description.contains("内存瓶颈"));
    }

    #[test]
    fn test_analyze_bottleneck_disk_io() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![
            sample(PerfMetricType::CpuUsage, 30.0),
            sample(PerfMetricType::MemoryUsage, 40.0),
            sample(PerfMetricType::DiskIoRead, 500.0),
            sample(PerfMetricType::DiskIoWrite, 500.0),
            sample(PerfMetricType::SwapUsage, 0.0),
            sample(PerfMetricType::LoadAverage, 2.0),
            sample(PerfMetricType::ProcessCount, 100.0),
        ];

        let bottleneck = monitor.analyze_bottleneck(&samples);
        // Disk IO score = min(500+500, 100) = 100, should be primary
        assert_eq!(bottleneck.primary, BottleneckType::DiskIoBound);
    }

    #[test]
    fn test_analyze_bottleneck_swap_thrashing() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![
            sample(PerfMetricType::CpuUsage, 30.0),
            sample(PerfMetricType::MemoryUsage, 40.0),
            sample(PerfMetricType::DiskIoRead, 10.0),
            sample(PerfMetricType::DiskIoWrite, 10.0),
            sample(PerfMetricType::SwapUsage, 80.0),
            sample(PerfMetricType::LoadAverage, 2.0),
            sample(PerfMetricType::ProcessCount, 100.0),
        ];

        let bottleneck = monitor.analyze_bottleneck(&samples);
        assert_eq!(bottleneck.primary, BottleneckType::SwapThrashing);
        assert!(bottleneck.description.contains("Swap"));
    }

    #[test]
    fn test_analyze_bottleneck_process_saturation() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![
            sample(PerfMetricType::CpuUsage, 30.0),
            sample(PerfMetricType::MemoryUsage, 40.0),
            sample(PerfMetricType::DiskIoRead, 10.0),
            sample(PerfMetricType::DiskIoWrite, 10.0),
            sample(PerfMetricType::SwapUsage, 0.0),
            sample(PerfMetricType::LoadAverage, 2.0),
            sample(PerfMetricType::ProcessCount, 800.0),
        ];

        let bottleneck = monitor.analyze_bottleneck(&samples);
        assert_eq!(bottleneck.primary, BottleneckType::ProcessSaturation);
        assert!(bottleneck.description.contains("进程饱和"));
    }

    #[test]
    fn test_analyze_bottleneck_secondary() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![
            sample(PerfMetricType::CpuUsage, 90.0),
            sample(PerfMetricType::MemoryUsage, 85.0),
            sample(PerfMetricType::DiskIoRead, 10.0),
            sample(PerfMetricType::DiskIoWrite, 10.0),
            sample(PerfMetricType::SwapUsage, 0.0),
            sample(PerfMetricType::LoadAverage, 2.0),
            sample(PerfMetricType::ProcessCount, 100.0),
        ];

        let bottleneck = monitor.analyze_bottleneck(&samples);
        assert_eq!(bottleneck.primary, BottleneckType::CpuBound);
        assert!(bottleneck.secondary.is_some());
    }

    #[test]
    fn test_generate_recommendations_cpu_bound() {
        let monitor = PerfMonitor::with_defaults();
        let bottleneck = BottleneckAnalysis {
            primary: BottleneckType::CpuBound,
            secondary: None,
            cpu_score: 95.0,
            memory_score: 30.0,
            disk_io_score: 10.0,
            network_score: 5.0,
            description: "CPU瓶颈".to_string(),
        };
        let samples = vec![sample(PerfMetricType::LoadAverage, 2.0)];

        let recs = monitor.generate_recommendations(&bottleneck, &samples);
        assert!(!recs.is_empty());
        assert!(recs.iter().any(|r| r.title.contains("CPU")));
        assert!(recs.iter().all(|r| r.command.is_some()));
    }

    #[test]
    fn test_generate_recommendations_memory_bound() {
        let monitor = PerfMonitor::with_defaults();
        let bottleneck = BottleneckAnalysis {
            primary: BottleneckType::MemoryBound,
            secondary: None,
            cpu_score: 30.0,
            memory_score: 92.0,
            disk_io_score: 10.0,
            network_score: 5.0,
            description: "内存瓶颈".to_string(),
        };
        let samples = vec![sample(PerfMetricType::LoadAverage, 2.0)];

        let recs = monitor.generate_recommendations(&bottleneck, &samples);
        assert!(recs
            .iter()
            .any(|r| r.title.contains("内存") || r.title.contains("缓存")));
    }

    #[test]
    fn test_generate_recommendations_no_bottleneck() {
        let monitor = PerfMonitor::with_defaults();
        let bottleneck = BottleneckAnalysis {
            primary: BottleneckType::NoBottleneck,
            secondary: None,
            cpu_score: 30.0,
            memory_score: 40.0,
            disk_io_score: 10.0,
            network_score: 5.0,
            description: "系统运行正常".to_string(),
        };
        let samples = vec![sample(PerfMetricType::LoadAverage, 1.0)];

        let recs = monitor.generate_recommendations(&bottleneck, &samples);
        assert!(!recs.is_empty());
        assert!(recs.iter().any(|r| r.title.contains("监控")));
    }

    #[test]
    fn test_generate_recommendations_high_load() {
        let monitor = PerfMonitor::with_defaults();
        let bottleneck = BottleneckAnalysis {
            primary: BottleneckType::NoBottleneck,
            secondary: None,
            cpu_score: 50.0,
            memory_score: 40.0,
            disk_io_score: 10.0,
            network_score: 5.0,
            description: "系统运行正常".to_string(),
        };
        let samples = vec![sample(PerfMetricType::LoadAverage, 8.0)];

        let recs = monitor.generate_recommendations(&bottleneck, &samples);
        assert!(recs.iter().any(|r| r.title.contains("负载")));
    }

    #[test]
    fn test_generate_recommendations_swap_thrashing() {
        let monitor = PerfMonitor::with_defaults();
        let bottleneck = BottleneckAnalysis {
            primary: BottleneckType::SwapThrashing,
            secondary: None,
            cpu_score: 30.0,
            memory_score: 50.0,
            disk_io_score: 10.0,
            network_score: 5.0,
            description: "Swap抖动".to_string(),
        };
        let samples = vec![];

        let recs = monitor.generate_recommendations(&bottleneck, &samples);
        assert!(recs.iter().any(|r| r.title.contains("Swap")));
        assert!(recs.iter().any(|r| r.title.contains("swappiness")));
    }

    #[test]
    fn test_generate_recommendations_disk_io() {
        let monitor = PerfMonitor::with_defaults();
        let bottleneck = BottleneckAnalysis {
            primary: BottleneckType::DiskIoBound,
            secondary: None,
            cpu_score: 30.0,
            memory_score: 40.0,
            disk_io_score: 95.0,
            network_score: 5.0,
            description: "磁盘IO瓶颈".to_string(),
        };
        let samples = vec![];

        let recs = monitor.generate_recommendations(&bottleneck, &samples);
        assert!(recs
            .iter()
            .any(|r| r.title.contains("IO") || r.title.contains("磁盘")));
    }

    #[test]
    fn test_generate_recommendations_network_bound() {
        let monitor = PerfMonitor::with_defaults();
        let bottleneck = BottleneckAnalysis {
            primary: BottleneckType::NetworkBound,
            secondary: None,
            cpu_score: 30.0,
            memory_score: 40.0,
            disk_io_score: 10.0,
            network_score: 90.0,
            description: "网络瓶颈".to_string(),
        };
        let samples = vec![];

        let recs = monitor.generate_recommendations(&bottleneck, &samples);
        assert!(recs.iter().any(|r| r.title.contains("网络")));
    }

    #[test]
    fn test_generate_recommendations_process_saturation() {
        let monitor = PerfMonitor::with_defaults();
        let bottleneck = BottleneckAnalysis {
            primary: BottleneckType::ProcessSaturation,
            secondary: None,
            cpu_score: 30.0,
            memory_score: 40.0,
            disk_io_score: 10.0,
            network_score: 5.0,
            description: "进程饱和".to_string(),
        };
        let samples = vec![];

        let recs = monitor.generate_recommendations(&bottleneck, &samples);
        assert!(recs.iter().any(|r| r.title.contains("进程")));
    }

    #[test]
    fn test_anomaly_severity_equality() {
        assert_eq!(AnomalySeverity::Info, AnomalySeverity::Info);
        assert_eq!(AnomalySeverity::Warning, AnomalySeverity::Warning);
        assert_eq!(AnomalySeverity::Critical, AnomalySeverity::Critical);
        assert_ne!(AnomalySeverity::Info, AnomalySeverity::Warning);
    }

    #[test]
    fn test_bottleneck_type_equality() {
        assert_eq!(BottleneckType::CpuBound, BottleneckType::CpuBound);
        assert_eq!(BottleneckType::NoBottleneck, BottleneckType::NoBottleneck);
        assert_ne!(BottleneckType::CpuBound, BottleneckType::MemoryBound);
    }

    #[test]
    fn test_perf_metric_type_variants() {
        let variants = [
            PerfMetricType::CpuUsage,
            PerfMetricType::MemoryUsage,
            PerfMetricType::DiskIoRead,
            PerfMetricType::DiskIoWrite,
            PerfMetricType::NetworkRx,
            PerfMetricType::NetworkTx,
            PerfMetricType::LoadAverage,
            PerfMetricType::SwapUsage,
            PerfMetricType::DiskUsage,
            PerfMetricType::ProcessCount,
            PerfMetricType::ContextSwitches,
            PerfMetricType::Interrupts,
        ];
        assert_eq!(variants.len(), 12);
    }

    #[test]
    fn test_perf_sample_serialization() {
        let s = sample(PerfMetricType::CpuUsage, 75.5);
        let json = serde_json::to_string(&s).unwrap();
        let deserialized: PerfSample = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.value, 75.5);
        assert_eq!(deserialized.metric_type, PerfMetricType::CpuUsage);
    }

    #[test]
    fn test_perf_baseline_serialization() {
        let bl = baseline(PerfMetricType::MemoryUsage, 60.0, 10.0);
        let json = serde_json::to_string(&bl).unwrap();
        let deserialized: PerfBaseline = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.mean, 60.0);
        assert_eq!(deserialized.std_dev, 10.0);
    }

    #[test]
    fn test_bottleneck_analysis_serialization() {
        let ba = BottleneckAnalysis {
            primary: BottleneckType::CpuBound,
            secondary: Some(BottleneckType::MemoryBound),
            cpu_score: 90.0,
            memory_score: 80.0,
            disk_io_score: 20.0,
            network_score: 10.0,
            description: "CPU瓶颈".to_string(),
        };
        let json = serde_json::to_string(&ba).unwrap();
        let deserialized: BottleneckAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.primary, BottleneckType::CpuBound);
        assert_eq!(deserialized.cpu_score, 90.0);
    }

    #[test]
    fn test_perf_report_serialization() {
        let report = PerfReport {
            host: "server1".to_string(),
            collected_at: Utc::now(),
            samples: vec![sample(PerfMetricType::CpuUsage, 50.0)],
            baselines: vec![],
            anomalies: vec![],
            bottleneck: BottleneckAnalysis {
                primary: BottleneckType::NoBottleneck,
                secondary: None,
                cpu_score: 50.0,
                memory_score: 40.0,
                disk_io_score: 10.0,
                network_score: 5.0,
                description: "正常".to_string(),
            },
            recommendations: vec![],
            overall_score: 90.0,
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: PerfReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host, "server1");
        assert_eq!(deserialized.overall_score, 90.0);
    }

    #[test]
    fn test_optimization_recommendation_serialization() {
        let rec = OptimizationRecommendation {
            category: BottleneckType::CpuBound,
            priority: 1,
            title: "测试建议".to_string(),
            description: "测试描述".to_string(),
            expected_improvement: "50%提升".to_string(),
            command: Some("top".to_string()),
        };
        let json = serde_json::to_string(&rec).unwrap();
        let deserialized: OptimizationRecommendation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.priority, 1);
        assert_eq!(deserialized.command, Some("top".to_string()));
    }

    #[test]
    fn test_perf_monitor_config_serialization() {
        let config = PerfMonitorConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: PerfMonitorConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.interval_secs, 60);
    }

    #[test]
    fn test_baseline_statistics_correctness() {
        let monitor = PerfMonitor::with_defaults();
        // Use known values: 10, 20, 30, ..., 100
        let samples: Vec<PerfSample> = (1..=100)
            .map(|i| sample(PerfMetricType::CpuUsage, i as f64))
            .collect();

        let bl = monitor
            .establish_baseline(&samples, &PerfMetricType::CpuUsage)
            .unwrap();
        // Mean of 1..=100 is 50.5
        assert!((bl.mean - 50.5).abs() < 0.1);
        // Min should be 1, max should be 100
        assert!((bl.min - 1.0).abs() < 0.01);
        assert!((bl.max - 100.0).abs() < 0.01);
        assert_eq!(bl.sample_count, 100);
    }

    #[test]
    fn test_detect_anomalies_boundary_warning() {
        let monitor = PerfMonitor::with_defaults();
        // Exactly at 2.0σ threshold
        let samples = vec![sample(PerfMetricType::CpuUsage, 60.0)];
        let baselines = vec![baseline(PerfMetricType::CpuUsage, 50.0, 5.0)];
        // deviation = |60-50|/5 = 2.0

        let anomalies = monitor.detect_anomalies(&samples, &baselines);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].severity, AnomalySeverity::Warning);
    }

    #[test]
    fn test_detect_anomalies_below_baseline() {
        let monitor = PerfMonitor::with_defaults();
        // Value significantly below baseline
        let samples = vec![sample(PerfMetricType::CpuUsage, 20.0)];
        let baselines = vec![baseline(PerfMetricType::CpuUsage, 50.0, 5.0)];
        // deviation = |20-50|/5 = 6.0σ

        let anomalies = monitor.detect_anomalies(&samples, &baselines);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].severity, AnomalySeverity::Critical);
    }

    #[test]
    fn test_bottleneck_score_calculation() {
        let monitor = PerfMonitor::with_defaults();
        let samples = vec![
            sample(PerfMetricType::CpuUsage, 75.0),
            sample(PerfMetricType::MemoryUsage, 60.0),
            sample(PerfMetricType::DiskIoRead, 30.0),
            sample(PerfMetricType::DiskIoWrite, 20.0),
            sample(PerfMetricType::NetworkRx, 100.0),
            sample(PerfMetricType::NetworkTx, 50.0),
            sample(PerfMetricType::SwapUsage, 10.0),
            sample(PerfMetricType::LoadAverage, 3.0),
            sample(PerfMetricType::ProcessCount, 200.0),
        ];

        let ba = monitor.analyze_bottleneck(&samples);
        assert_eq!(ba.cpu_score, 75.0);
        assert_eq!(ba.memory_score, 60.0);
        assert_eq!(ba.disk_io_score, 50.0); // 30+20=50
                                            // network: (100+50)/10 = 15.0
        assert!((ba.network_score - 15.0).abs() < 0.1);
    }
}
