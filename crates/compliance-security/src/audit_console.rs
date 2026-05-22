//! # Audit Console Data
//!
//! Provides structured audit visualization data for compliance dashboards.
//! Transforms raw audit events into chart-ready formats for:
//!
//! - **Timeline Charts** — events over time (compliance violations, approvals)
//! - **Category Pie/Donut Charts** — breakdown by event type
//! - **Risk Heatmaps** — risk intensity by agent or time bucket
//! - **Summary Stat Cards** — KPI metrics (total events, violation rate, etc.)
//!
//! ## Design
//!
//! ```text
//! AuditEvent / AuditRecord ──► AuditVisualizer ──► ChartData
//!                                           ├── TimelineData
//!                                           ├── CategoryData
//!                                           ├── HeatmapData
//!                                           └── StatCardData
//! ```

use chrono::{DateTime, Datelike, Months, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A raw audit event from the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub agent_id: Option<String>,
    pub user_id: Option<String>,
    pub action: String,
    pub resource: String,
    pub outcome: AuditOutcome,
    pub severity: AuditSeverity,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditEventType {
    PolicyViolation,
    ApprovalRequest,
    ToolInvocation,
    DataAccess,
    AgentSpawn,
    AgentTermination,
    ConfigurationChange,
    Authentication,
    Authorization,
    ComplianceCheck,
    RedTeamRun,
    ManualOverride,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditEventType::PolicyViolation => write!(f, "policy_violation"),
            AuditEventType::ApprovalRequest => write!(f, "approval_request"),
            AuditEventType::ToolInvocation => write!(f, "tool_invocation"),
            AuditEventType::DataAccess => write!(f, "data_access"),
            AuditEventType::AgentSpawn => write!(f, "agent_spawn"),
            AuditEventType::AgentTermination => write!(f, "agent_termination"),
            AuditEventType::ConfigurationChange => write!(f, "configuration_change"),
            AuditEventType::Authentication => write!(f, "authentication"),
            AuditEventType::Authorization => write!(f, "authorization"),
            AuditEventType::ComplianceCheck => write!(f, "compliance_check"),
            AuditEventType::RedTeamRun => write!(f, "red_team_run"),
            AuditEventType::ManualOverride => write!(f, "manual_override"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditOutcome {
    Success,
    Denied,
    Warning,
    Pending,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl AuditSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditSeverity::Info => "info",
            AuditSeverity::Low => "low",
            AuditSeverity::Medium => "medium",
            AuditSeverity::High => "high",
            AuditSeverity::Critical => "critical",
        }
    }

    pub fn numeric(&self) -> u8 {
        match self {
            AuditSeverity::Info => 1,
            AuditSeverity::Low => 2,
            AuditSeverity::Medium => 3,
            AuditSeverity::High => 4,
            AuditSeverity::Critical => 5,
        }
    }
}

// ─── Chart Data Structures ────────────────────────────────────────────────────

/// Data point for a timeline chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelinePoint {
    /// ISO-8601 timestamp
    pub timestamp: String,
    /// Number of events in this bucket
    pub count: f64,
    /// Optional breakdown by sub-category
    pub breakdown: HashMap<String, f64>,
}

/// Timeline chart data ready for visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineData {
    pub label: String,
    pub description: String,
    pub unit: String,
    pub points: Vec<TimelinePoint>,
    pub min_value: f64,
    pub max_value: f64,
    /// Total count across all points
    pub total: f64,
}

/// Data point for a category (pie/donut) chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryPoint {
    pub label: String,
    pub value: f64,
    pub percentage: f64,
}

/// Category chart data ready for visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryData {
    pub label: String,
    pub description: String,
    pub points: Vec<CategoryPoint>,
    pub total: f64,
}

/// A single cell in a heatmap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapCell {
    pub row: String,
    pub column: String,
    pub value: f64,
    pub intensity: f64, // 0.0–1.0 normalized
}

/// Heatmap data ready for visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapData {
    pub label: String,
    pub row_labels: Vec<String>,
    pub column_labels: Vec<String>,
    pub cells: Vec<HeatmapCell>,
    pub min_value: f64,
    pub max_value: f64,
}

/// A single KPI stat card value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatCard {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub formatted_value: String,
    /// Change from previous period (positive = increase, negative = decrease)
    pub change: Option<f64>,
    pub change_direction: Option<ChangeDirection>,
    pub trend: TrendDirection,
    pub severity: Option<AuditSeverity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeDirection {
    Up,
    Down,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
}

/// All stat cards for a dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatCardsData {
    pub cards: Vec<StatCard>,
    pub period_label: String,
    pub comparison_period_label: Option<String>,
}

/// Combined audit visualization data for a dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditDashboardData {
    pub timeline: TimelineData,
    pub category_breakdown: CategoryData,
    pub severity_breakdown: CategoryData,
    pub agent_heatmap: Option<HeatmapData>,
    pub stat_cards: StatCardsData,
    pub generated_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

// ─── AuditVisualizer ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeBucket {
    Hour,
    Day,
    Week,
    Month,
}

impl TimeBucket {
    fn bucket_key(&self, dt: &DateTime<Utc>) -> String {
        match self {
            TimeBucket::Hour => format!(
                "{:04}-{:02}-{:02}T{:02}:00",
                dt.year(),
                dt.month(),
                dt.day(),
                dt.hour()
            ),
            TimeBucket::Day => format!("{:04}-{:02}-{:02}", dt.year(), dt.month(), dt.day()),
            TimeBucket::Week => format!("{:04}-W{:02}", dt.year(), dt.iso_week().week()),
            TimeBucket::Month => format!("{:04}-{:02}", dt.year(), dt.month()),
        }
    }
}

/// Converts raw audit events into visualization-ready data.
pub struct AuditVisualizer {
    events: Vec<AuditEvent>,
    time_bucket: TimeBucket,
}

impl AuditVisualizer {
    pub fn new(events: Vec<AuditEvent>) -> Self {
        Self {
            events,
            time_bucket: TimeBucket::Day,
        }
    }

    pub fn with_time_bucket(mut self, bucket: TimeBucket) -> Self {
        self.time_bucket = bucket;
        self
    }

    /// Generate timeline data for a specific event type.
    pub fn timeline_for_event_type(&self, event_type: &AuditEventType) -> TimelineData {
        let filtered: Vec<_> = self
            .events
            .iter()
            .filter(|e| &e.event_type == event_type)
            .collect();

        self.build_timeline(&filtered)
    }

    /// Generate timeline data for all events.
    pub fn timeline_all_events(&self) -> TimelineData {
        let refs: Vec<_> = self.events.iter().collect();
        self.build_timeline(&refs)
    }

    fn build_timeline(&self, events: &[&AuditEvent]) -> TimelineData {
        let mut buckets: HashMap<String, HashMap<String, f64>> = HashMap::new();
        let mut all_counts: Vec<f64> = Vec::new();

        for event in events {
            let key = self.time_bucket.bucket_key(&event.timestamp);
            let counter = buckets.entry(key).or_insert_with(HashMap::new);
            *counter.entry(event.event_type.to_string()).or_insert(0.0) += 1.0;
        }

        let mut points: Vec<TimelinePoint> = buckets
            .into_iter()
            .map(|(timestamp, breakdown)| {
                let count = breakdown.values().sum();
                all_counts.push(count);
                TimelinePoint {
                    timestamp,
                    count,
                    breakdown,
                }
            })
            .collect();

        points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        let min_value = all_counts.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_value = all_counts.iter().cloned().fold(0.0, f64::max);
        let total = all_counts.iter().sum::<f64>();

        TimelineData {
            label: "Events Over Time".to_string(),
            description: format!("{:?} bucketed event timeline", self.time_bucket),
            unit: "events".to_string(),
            points,
            min_value: if min_value.is_infinite() {
                0.0
            } else {
                min_value
            },
            max_value,
            total,
        }
    }

    /// Generate category (pie/donut) breakdown by event type.
    pub fn category_by_event_type(&self) -> CategoryData {
        let mut counts: HashMap<String, f64> = HashMap::new();
        for event in &self.events {
            *counts.entry(event.event_type.to_string()).or_insert(0.0) += 1.0;
        }
        self.build_category_data(
            counts,
            "Events by Type",
            "Breakdown of audit events by category",
        )
    }

    /// Generate category breakdown by severity.
    pub fn category_by_severity(&self) -> CategoryData {
        let mut counts: HashMap<String, f64> = HashMap::new();
        for event in &self.events {
            *counts
                .entry(event.severity.as_str().to_string())
                .or_insert(0.0) += 1.0;
        }
        self.build_category_data(
            counts,
            "Events by Severity",
            "Breakdown of audit events by severity level",
        )
    }

    /// Generate category breakdown by outcome.
    pub fn category_by_outcome(&self) -> CategoryData {
        let mut counts: HashMap<String, f64> = HashMap::new();
        for event in &self.events {
            let outcome_str = match event.outcome {
                AuditOutcome::Success => "success",
                AuditOutcome::Denied => "denied",
                AuditOutcome::Warning => "warning",
                AuditOutcome::Pending => "pending",
                AuditOutcome::Error => "error",
            };
            *counts.entry(outcome_str.to_string()).or_insert(0.0) += 1.0;
        }
        self.build_category_data(
            counts,
            "Events by Outcome",
            "Breakdown of audit events by outcome",
        )
    }

    fn build_category_data(
        &self,
        counts: HashMap<String, f64>,
        label: &str,
        desc: &str,
    ) -> CategoryData {
        let total = counts.values().sum();
        let mut points: Vec<CategoryPoint> = counts
            .into_iter()
            .map(|(label, value)| CategoryPoint {
                label,
                value,
                percentage: if total > 0.0 {
                    (value / total) * 100.0
                } else {
                    0.0
                },
            })
            .collect();
        points.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap());

        CategoryData {
            label: label.to_string(),
            description: desc.to_string(),
            points,
            total,
        }
    }

    /// Generate heatmap of violations by agent and hour-of-day.
    pub fn heatmap_by_agent_and_hour(&self) -> Option<HeatmapData> {
        let filtered: Vec<_> = self
            .events
            .iter()
            .filter(|e| {
                e.agent_id.is_some()
                    && matches!(e.outcome, AuditOutcome::Denied | AuditOutcome::Warning)
            })
            .collect();

        if filtered.is_empty() {
            return None;
        }

        // Collect unique agents
        let mut agents: Vec<_> = filtered.iter().filter_map(|e| e.agent_id.clone()).collect();
        agents.sort();
        agents.dedup();

        let hours: Vec<String> = (0..24).map(|h| format!("{:02}:00", h)).collect();

        let mut cells_map: HashMap<(String, String), f64> = HashMap::new();
        for event in &filtered {
            if let Some(ref agent_id) = event.agent_id {
                let hour_key = format!("{:02}:00", event.timestamp.hour());
                *cells_map.entry((agent_id.clone(), hour_key)).or_insert(0.0) += 1.0;
            }
        }

        let max_value = cells_map.values().cloned().fold(0.0, f64::max);

        let cells: Vec<HeatmapCell> = cells_map
            .into_iter()
            .map(|((row, col), value)| HeatmapCell {
                row,
                column: col,
                value,
                intensity: if max_value > 0.0 {
                    value / max_value
                } else {
                    0.0
                },
            })
            .collect();

        Some(HeatmapData {
            label: "Violation Heatmap (Agent × Hour)".to_string(),
            row_labels: agents,
            column_labels: hours,
            cells,
            min_value: 0.0,
            max_value,
        })
    }

    /// Generate stat cards for key metrics.
    pub fn stat_cards(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> StatCardsData {
        let total_events = self.events.len() as f64;

        let denied = self
            .events
            .iter()
            .filter(|e| e.outcome == AuditOutcome::Denied)
            .count() as f64;
        let warnings = self
            .events
            .iter()
            .filter(|e| e.outcome == AuditOutcome::Warning)
            .count() as f64;
        let critical = self
            .events
            .iter()
            .filter(|e| e.severity == AuditSeverity::Critical)
            .count() as f64;
        let pending = self
            .events
            .iter()
            .filter(|e| e.outcome == AuditOutcome::Pending)
            .count() as f64;

        let violation_rate = if total_events > 0.0 {
            (denied + warnings) / total_events
        } else {
            0.0
        };
        let denial_rate = if total_events > 0.0 {
            denied / total_events
        } else {
            0.0
        };

        let cards = vec![
            StatCard {
                id: "total_events".to_string(),
                label: "Total Events".to_string(),
                value: total_events,
                formatted_value: format!("{:.0}", total_events),
                change: None,
                change_direction: None,
                trend: TrendDirection::Stable,
                severity: None,
            },
            StatCard {
                id: "denied_count".to_string(),
                label: "Denied Actions".to_string(),
                value: denied,
                formatted_value: format!("{:.0}", denied),
                change: None,
                change_direction: None,
                trend: TrendDirection::Stable,
                severity: Some(AuditSeverity::High),
            },
            StatCard {
                id: "violation_rate".to_string(),
                label: "Violation Rate".to_string(),
                value: violation_rate,
                formatted_value: format!("{:.1}%", violation_rate * 100.0),
                change: None,
                change_direction: None,
                trend: TrendDirection::Stable,
                severity: if violation_rate > 0.1 {
                    Some(AuditSeverity::High)
                } else {
                    None
                },
            },
            StatCard {
                id: "denial_rate".to_string(),
                label: "Denial Rate".to_string(),
                value: denial_rate,
                formatted_value: format!("{:.1}%", denial_rate * 100.0),
                change: None,
                change_direction: None,
                trend: TrendDirection::Stable,
                severity: if denial_rate > 0.05 {
                    Some(AuditSeverity::Medium)
                } else {
                    None
                },
            },
            StatCard {
                id: "critical_events".to_string(),
                label: "Critical Events".to_string(),
                value: critical,
                formatted_value: format!("{:.0}", critical),
                change: None,
                change_direction: None,
                trend: TrendDirection::Stable,
                severity: if critical > 0.0 {
                    Some(AuditSeverity::Critical)
                } else {
                    None
                },
            },
            StatCard {
                id: "pending_approvals".to_string(),
                label: "Pending Approvals".to_string(),
                value: pending,
                formatted_value: format!("{:.0}", pending),
                change: None,
                change_direction: None,
                trend: TrendDirection::Stable,
                severity: None,
            },
        ];

        StatCardsData {
            cards,
            period_label: format!(
                "{} to {}",
                period_start.to_rfc3339(),
                period_end.to_rfc3339()
            ),
            comparison_period_label: None,
        }
    }

    /// Generate a complete dashboard data set.
    pub fn build_dashboard(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> AuditDashboardData {
        AuditDashboardData {
            timeline: self.timeline_all_events(),
            category_breakdown: self.category_by_event_type(),
            severity_breakdown: self.category_by_severity(),
            agent_heatmap: self.heatmap_by_agent_and_hour(),
            stat_cards: self.stat_cards(period_start, period_end),
            generated_at: Utc::now(),
            period_start,
            period_end,
        }
    }
}

impl Default for AuditVisualizer {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_events() -> Vec<AuditEvent> {
        let base = Utc::now();
        vec![
            AuditEvent {
                id: "e1".to_string(),
                timestamp: base,
                event_type: AuditEventType::PolicyViolation,
                agent_id: Some("agent-1".to_string()),
                user_id: Some("user-1".to_string()),
                action: "data.read".to_string(),
                resource: "audit_log".to_string(),
                outcome: AuditOutcome::Denied,
                severity: AuditSeverity::High,
                metadata: HashMap::new(),
            },
            AuditEvent {
                id: "e2".to_string(),
                timestamp: base + Duration::hours(1),
                event_type: AuditEventType::ToolInvocation,
                agent_id: Some("agent-1".to_string()),
                user_id: Some("user-1".to_string()),
                action: "tool.call".to_string(),
                resource: "read_file".to_string(),
                outcome: AuditOutcome::Success,
                severity: AuditSeverity::Info,
                metadata: HashMap::new(),
            },
            AuditEvent {
                id: "e3".to_string(),
                timestamp: base + Duration::hours(2),
                event_type: AuditEventType::ApprovalRequest,
                agent_id: None,
                user_id: Some("user-2".to_string()),
                action: "agent.spawn".to_string(),
                resource: "new-agent".to_string(),
                outcome: AuditOutcome::Pending,
                severity: AuditSeverity::Medium,
                metadata: HashMap::new(),
            },
            AuditEvent {
                id: "e4".to_string(),
                timestamp: base,
                event_type: AuditEventType::PolicyViolation,
                agent_id: Some("agent-2".to_string()),
                user_id: Some("user-1".to_string()),
                action: "data.export".to_string(),
                resource: "credentials".to_string(),
                outcome: AuditOutcome::Denied,
                severity: AuditSeverity::Critical,
                metadata: HashMap::new(),
            },
        ]
    }

    // --- AuditEventType display ---

    #[test]
    fn test_audit_event_type_display() {
        assert_eq!(
            AuditEventType::PolicyViolation.to_string(),
            "policy_violation"
        );
        assert_eq!(
            AuditEventType::ToolInvocation.to_string(),
            "tool_invocation"
        );
        assert_eq!(AuditEventType::RedTeamRun.to_string(), "red_team_run");
    }

    // --- AuditSeverity tests ---

    #[test]
    fn test_audit_severity_ordering() {
        assert!(AuditSeverity::Critical > AuditSeverity::High);
        assert!(AuditSeverity::High > AuditSeverity::Medium);
        assert!(AuditSeverity::Medium > AuditSeverity::Low);
        assert!(AuditSeverity::Low > AuditSeverity::Info);
    }

    #[test]
    fn test_audit_severity_numeric() {
        assert_eq!(AuditSeverity::Info.numeric(), 1);
        assert_eq!(AuditSeverity::Critical.numeric(), 5);
    }

    // --- AuditVisualizer timeline tests ---

    #[test]
    fn test_visualizer_timeline_all_events() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let timeline = viz.timeline_all_events();

        assert!(!timeline.points.is_empty());
        assert!(timeline.total > 0.0);
        assert!(timeline.max_value >= timeline.min_value);
    }

    #[test]
    fn test_visualizer_timeline_filtered_by_event_type() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let timeline = viz.timeline_for_event_type(&AuditEventType::PolicyViolation);

        // Only policy violations
        for point in &timeline.points {
            assert!(point.breakdown.contains_key("policy_violation"));
        }
    }

    #[test]
    fn test_visualizer_empty_events() {
        let viz = AuditVisualizer::new(vec![]);
        let timeline = viz.timeline_all_events();
        assert_eq!(timeline.total, 0.0);
        assert!(timeline.points.is_empty());
    }

    // --- Category breakdown tests ---

    #[test]
    fn test_category_by_event_type() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let cat = viz.category_by_event_type();

        assert_eq!(cat.points.len(), 3); // PolicyViolation, ToolInvocation, ApprovalRequest
        assert_eq!(cat.total, 4.0);
        // PolicyViolation should have 2
        let pv = cat
            .points
            .iter()
            .find(|p| p.label == "policy_violation")
            .unwrap();
        assert_eq!(pv.value, 2.0);
    }

    #[test]
    fn test_category_by_severity() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let cat = viz.category_by_severity();

        // Should have 3 severity levels: High, Info, Medium, Critical
        assert_eq!(cat.points.len(), 4);
        let critical = cat.points.iter().find(|p| p.label == "critical").unwrap();
        assert_eq!(critical.value, 1.0);
    }

    #[test]
    fn test_category_percentages_sum_to_100() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let cat = viz.category_by_event_type();

        let sum: f64 = cat.points.iter().map(|p| p.percentage).sum();
        assert!((sum - 100.0).abs() < 0.01);
    }

    // --- Heatmap tests ---

    #[test]
    fn test_heatmap_generated() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let heatmap = viz.heatmap_by_agent_and_hour();

        assert!(heatmap.is_some());
        let h = heatmap.unwrap();
        assert!(!h.cells.is_empty());
        assert!(h.max_value >= h.min_value);
    }

    #[test]
    fn test_heatmap_intensity_bounded() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let heatmap = viz.heatmap_by_agent_and_hour().unwrap();

        for cell in &heatmap.cells {
            assert!(cell.intensity >= 0.0 && cell.intensity <= 1.0);
        }
    }

    #[test]
    fn test_heatmap_empty_when_no_violations() {
        let viz = AuditVisualizer::new(vec![]);
        assert!(viz.heatmap_by_agent_and_hour().is_none());
    }

    // --- Stat cards tests ---

    #[test]
    fn test_stat_cards_values() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let now = Utc::now();
        let cards = viz.stat_cards(now - Duration::days(1), now);

        let total_card = cards.cards.iter().find(|c| c.id == "total_events").unwrap();
        assert_eq!(total_card.value, 4.0);

        let denied_card = cards.cards.iter().find(|c| c.id == "denied_count").unwrap();
        assert_eq!(denied_card.value, 2.0);
    }

    #[test]
    fn test_stat_cards_violation_rate() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let now = Utc::now();
        let cards = viz.stat_cards(now - Duration::days(1), now);

        let vr = cards
            .cards
            .iter()
            .find(|c| c.id == "violation_rate")
            .unwrap();
        // 2 violations (denied) out of 4 events = 50%
        assert!((vr.value - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_stat_cards_formatted() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let now = Utc::now();
        let cards = viz.stat_cards(now - Duration::days(1), now);

        let total_card = cards.cards.iter().find(|c| c.id == "total_events").unwrap();
        assert_eq!(total_card.formatted_value, "4");
    }

    // --- Dashboard build tests ---

    #[test]
    fn test_build_dashboard_complete() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let now = Utc::now();
        let dashboard = viz.build_dashboard(now - Duration::days(1), now);

        assert!(!dashboard.timeline.points.is_empty());
        assert!(!dashboard.category_breakdown.points.is_empty());
        assert!(!dashboard.stat_cards.cards.is_empty());
        assert!(dashboard.generated_at <= Utc::now());
    }

    #[test]
    fn test_dashboard_period_bounds() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let start = Utc::now() - Duration::days(7);
        let end = Utc::now();
        let dashboard = viz.build_dashboard(start, end);

        assert!(dashboard.period_start <= dashboard.period_end);
        assert_eq!(dashboard.period_start, start);
        assert_eq!(dashboard.period_end, end);
    }

    // --- Serde tests ---

    #[test]
    fn test_audit_event_serde() {
        let event = make_events()[0].clone();
        let json = serde_json::to_string(&event).unwrap();
        let decoded: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "e1");
        assert_eq!(decoded.outcome, AuditOutcome::Denied);
    }

    #[test]
    fn test_timeline_data_serde() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let timeline = viz.timeline_all_events();
        let json = serde_json::to_string(&timeline).unwrap();
        let decoded: TimelineData = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.total, timeline.total);
    }

    #[test]
    fn test_category_data_serde() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let cat = viz.category_by_event_type();
        let json = serde_json::to_string(&cat).unwrap();
        let decoded: CategoryData = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.points.len(), cat.points.len());
    }

    #[test]
    fn test_stat_card_serde() {
        let cards = StatCardsData {
            cards: vec![StatCard {
                id: "test".to_string(),
                label: "Test Card".to_string(),
                value: 42.0,
                formatted_value: "42".to_string(),
                change: Some(5.0),
                change_direction: Some(ChangeDirection::Up),
                trend: TrendDirection::Increasing,
                severity: Some(AuditSeverity::Medium),
            }],
            period_label: "period".to_string(),
            comparison_period_label: Some("prev".to_string()),
        };
        let json = serde_json::to_string(&cards).unwrap();
        let decoded: StatCardsData = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.cards[0].value, 42.0);
    }

    #[test]
    fn test_heatmap_cell_serde() {
        let cell = HeatmapCell {
            row: "agent-1".to_string(),
            column: "14:00".to_string(),
            value: 3.0,
            intensity: 0.75,
        };
        let json = serde_json::to_string(&cell).unwrap();
        let decoded: HeatmapCell = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.row, "agent-1");
    }

    #[test]
    fn test_audit_dashboard_data_serde() {
        let events = make_events();
        let viz = AuditVisualizer::new(events);
        let now = Utc::now();
        let dashboard = viz.build_dashboard(now - Duration::days(1), now);

        let json = serde_json::to_string(&dashboard).unwrap();
        let decoded: AuditDashboardData = serde_json::from_str(&json).unwrap();
        assert_eq!(
            decoded.stat_cards.cards.len(),
            dashboard.stat_cards.cards.len()
        );
    }

    // --- TimeBucket tests ---

    #[test]
    fn test_time_bucket_hour_key() {
        let dt = Utc::now();
        assert_eq!(TimeBucket::Hour.bucket_key(&dt).len(), 16); // YYYY-MM-DDTHH:00
    }

    #[test]
    fn test_time_bucket_day_key() {
        let dt = Utc::now();
        assert_eq!(TimeBucket::Day.bucket_key(&dt).len(), 10); // YYYY-MM-DD
    }

    #[test]
    fn test_time_bucket_month_key() {
        let dt = Utc::now();
        let key = TimeBucket::Month.bucket_key(&dt);
        assert!(key.contains('-'));
        let parts: Vec<_> = key.split('-').collect();
        assert_eq!(parts[0].len(), 4); // Year
        assert_eq!(parts[1].len(), 2); // Month
    }

    // --- Empty and edge cases ---

    #[test]
    fn test_visualizer_with_zero_events() {
        let viz = AuditVisualizer::new(vec![]);
        let cat = viz.category_by_event_type();
        assert_eq!(cat.total, 0.0);
        assert!(cat.points.is_empty());
    }

    #[test]
    fn test_audit_dashboard_with_empty_events() {
        let viz = AuditVisualizer::new(vec![]);
        let now = Utc::now();
        let dashboard = viz.build_dashboard(now - Duration::days(1), now);
        assert_eq!(dashboard.timeline.total, 0.0);
    }
}
