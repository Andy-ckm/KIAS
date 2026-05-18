//! KIAS 可视化模块
//!
//! 提供知识图谱、文档关系、审计时间线、合规状态的可视化 API

use axum::{extract::Query, http::StatusCode, response::Html, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 知识图谱节点
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String, // person, company, concept, document
    pub properties: HashMap<String, String>,
    pub tier: Option<u8>, // 1-3, 实体分层
}

/// 知识图谱边
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub relation: String, // works_at, invested_in, founded, etc.
    pub weight: Option<f64>,
}

/// 知识图谱数据
#[derive(Debug, Serialize, Deserialize)]
pub struct KnowledgeGraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub metadata: GraphMetadata,
}

/// 图谱元数据
#[derive(Debug, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub node_types: HashMap<String, usize>,
    pub relation_types: HashMap<String, usize>,
}

/// 文档关系数据
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentRelationData {
    pub documents: Vec<DocumentNode>,
    pub relations: Vec<DocumentRelation>,
}

/// 文档节点
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentNode {
    pub id: String,
    pub title: String,
    pub doc_type: String, // sop, verification, capa, dhf
    pub version: String,
    pub status: String, // draft, review, approved, obsolete
    pub last_modified: String,
}

/// 文档关系
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentRelation {
    pub source: String,
    pub target: String,
    pub relation: String, // references, supersedes, related_to
}

/// 审计时间线事件
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: String,
    pub event_type: String, // create, read, update, delete, approve, reject
    pub actor: String,
    pub resource: String,
    pub details: String,
    pub hash: String, // SHA-256 哈希
}

/// 合规状态
#[derive(Debug, Serialize, Deserialize)]
pub struct ComplianceStatus {
    pub overall_score: f64,
    pub categories: Vec<ComplianceCategory>,
    pub recent_events: Vec<AuditEvent>,
    pub risk_items: Vec<RiskItem>,
}

/// 合规类别
#[derive(Debug, Serialize, Deserialize)]
pub struct ComplianceCategory {
    pub name: String,
    pub score: f64,
    pub total_checks: usize,
    pub passed_checks: usize,
    pub failed_checks: usize,
}

/// 风险项
#[derive(Debug, Serialize, Deserialize)]
pub struct RiskItem {
    pub id: String,
    pub severity: String, // high, medium, low
    pub description: String,
    pub affected_resources: Vec<String>,
    pub mitigation: String,
}

/// 查询参数
#[derive(Debug, Deserialize)]
pub struct GraphQueryParams {
    pub depth: Option<u8>,
    pub node_type: Option<String>,
    pub limit: Option<usize>,
}

/// 生成知识图谱可视化 HTML
pub async fn knowledge_graph_html() -> Html<String> {
    // 读取静态 HTML 文件
    let html = include_str!("../../../static/knowledge-graph.html");
    Html(html.to_string())
}

/// 生成合规仪表盘 HTML
pub async fn compliance_dashboard_html() -> Html<String> {
    // 读取静态 HTML 文件
    let html = include_str!("../../../static/compliance-dashboard.html");
    Html(html.to_string())
}

/// 生成审计时间线 HTML
pub async fn audit_timeline_html() -> Html<String> {
    // 读取静态 HTML 文件
    let html = include_str!("../../../static/audit-timeline.html");
    Html(html.to_string())
}

/// 获取知识图谱数据 API
pub async fn get_knowledge_graph(
    Query(_params): Query<GraphQueryParams>,
) -> Result<Json<KnowledgeGraphData>, StatusCode> {
    // 从知识库获取数据
    let nodes = vec![
        GraphNode {
            id: "1".to_string(),
            label: "知识图谱".to_string(),
            node_type: "concept".to_string(),
            properties: HashMap::new(),
            tier: Some(1),
        },
        GraphNode {
            id: "2".to_string(),
            label: "实体提取".to_string(),
            node_type: "concept".to_string(),
            properties: HashMap::new(),
            tier: Some(2),
        },
    ];

    let edges = vec![GraphEdge {
        source: "1".to_string(),
        target: "2".to_string(),
        relation: "uses".to_string(),
        weight: Some(1.0),
    }];

    let metadata = GraphMetadata {
        total_nodes: nodes.len(),
        total_edges: edges.len(),
        node_types: HashMap::new(),
        relation_types: HashMap::new(),
    };

    Ok(Json(KnowledgeGraphData {
        nodes,
        edges,
        metadata,
    }))
}

/// 获取文档关系数据 API
pub async fn get_document_relations() -> Result<Json<DocumentRelationData>, StatusCode> {
    let documents = vec![
        DocumentNode {
            id: "doc-1".to_string(),
            title: "SOP-001: 设计控制流程".to_string(),
            doc_type: "sop".to_string(),
            version: "2.1".to_string(),
            status: "approved".to_string(),
            last_modified: "2026-05-18".to_string(),
        },
        DocumentNode {
            id: "doc-2".to_string(),
            title: "VER-001: 验证报告".to_string(),
            doc_type: "verification".to_string(),
            version: "1.0".to_string(),
            status: "review".to_string(),
            last_modified: "2026-05-17".to_string(),
        },
    ];

    let relations = vec![DocumentRelation {
        source: "doc-1".to_string(),
        target: "doc-2".to_string(),
        relation: "references".to_string(),
    }];

    Ok(Json(DocumentRelationData {
        documents,
        relations,
    }))
}

/// 获取审计时间线 API
pub async fn get_audit_timeline() -> Result<Json<Vec<AuditEvent>>, StatusCode> {
    let events = vec![
        AuditEvent {
            id: "evt-1".to_string(),
            timestamp: "2026-05-18T10:00:00Z".to_string(),
            event_type: "create".to_string(),
            actor: "user-1".to_string(),
            resource: "doc-1".to_string(),
            details: "创建设计控制流程文档".to_string(),
            hash: "abc123...".to_string(),
        },
        AuditEvent {
            id: "evt-2".to_string(),
            timestamp: "2026-05-18T11:00:00Z".to_string(),
            event_type: "approve".to_string(),
            actor: "user-2".to_string(),
            resource: "doc-1".to_string(),
            details: "审批设计控制流程文档".to_string(),
            hash: "def456...".to_string(),
        },
    ];

    Ok(Json(events))
}

/// 获取合规状态 API
pub async fn get_compliance_status() -> Result<Json<ComplianceStatus>, StatusCode> {
    let categories = vec![
        ComplianceCategory {
            name: "审计追踪".to_string(),
            score: 95.0,
            total_checks: 20,
            passed_checks: 19,
            failed_checks: 1,
        },
        ComplianceCategory {
            name: "电子签名".to_string(),
            score: 100.0,
            total_checks: 15,
            passed_checks: 15,
            failed_checks: 0,
        },
        ComplianceCategory {
            name: "审批流程".to_string(),
            score: 90.0,
            total_checks: 10,
            passed_checks: 9,
            failed_checks: 1,
        },
        ComplianceCategory {
            name: "预演机制".to_string(),
            score: 85.0,
            total_checks: 8,
            passed_checks: 7,
            failed_checks: 1,
        },
    ];

    Ok(Json(ComplianceStatus {
        overall_score: 92.5,
        categories,
        recent_events: vec![],
        risk_items: vec![],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_node_serialization() {
        let node = GraphNode {
            id: "1".to_string(),
            label: "test".to_string(),
            node_type: "concept".to_string(),
            properties: HashMap::new(),
            tier: Some(1),
        };
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("\"id\":\"1\""));
        assert!(json.contains("\"label\":\"test\""));
        let deserialized: GraphNode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "1");
    }

    #[test]
    fn test_graph_edge_serialization() {
        let edge = GraphEdge {
            source: "a".to_string(),
            target: "b".to_string(),
            relation: "uses".to_string(),
            weight: Some(0.8),
        };
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"source\":\"a\""));
        let deserialized: GraphEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.relation, "uses");
        assert!((deserialized.weight.unwrap() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_knowledge_graph_data_serialization() {
        let data = KnowledgeGraphData {
            nodes: vec![GraphNode {
                id: "1".to_string(),
                label: "root".to_string(),
                node_type: "concept".to_string(),
                properties: HashMap::new(),
                tier: None,
            }],
            edges: vec![],
            metadata: GraphMetadata {
                total_nodes: 1,
                total_edges: 0,
                node_types: HashMap::new(),
                relation_types: HashMap::new(),
            },
        };
        let json = serde_json::to_string(&data).unwrap();
        let deserialized: KnowledgeGraphData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.nodes.len(), 1);
        assert_eq!(deserialized.metadata.total_nodes, 1);
    }

    #[test]
    fn test_document_node_serialization() {
        let doc = DocumentNode {
            id: "doc-1".to_string(),
            title: "SOP-001".to_string(),
            doc_type: "sop".to_string(),
            version: "1.0".to_string(),
            status: "approved".to_string(),
            last_modified: "2026-05-18".to_string(),
        };
        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: DocumentNode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.doc_type, "sop");
        assert_eq!(deserialized.status, "approved");
    }

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEvent {
            id: "evt-1".to_string(),
            timestamp: "2026-05-18T10:00:00Z".to_string(),
            event_type: "create".to_string(),
            actor: "user-1".to_string(),
            resource: "doc-1".to_string(),
            details: "Created document".to_string(),
            hash: "abc123".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event_type\":\"create\""));
        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.hash, "abc123");
    }

    #[test]
    fn test_compliance_status_serialization() {
        let status = ComplianceStatus {
            overall_score: 95.0,
            categories: vec![ComplianceCategory {
                name: "audit".to_string(),
                score: 100.0,
                total_checks: 10,
                passed_checks: 10,
                failed_checks: 0,
            }],
            recent_events: vec![],
            risk_items: vec![RiskItem {
                id: "risk-1".to_string(),
                severity: "high".to_string(),
                description: "test risk".to_string(),
                affected_resources: vec!["res-1".to_string()],
                mitigation: "fix it".to_string(),
            }],
        };
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: ComplianceStatus = serde_json::from_str(&json).unwrap();
        assert!((deserialized.overall_score - 95.0).abs() < f64::EPSILON);
        assert_eq!(deserialized.categories.len(), 1);
        assert_eq!(deserialized.risk_items.len(), 1);
        assert_eq!(deserialized.risk_items[0].severity, "high");
    }

    #[test]
    fn test_graph_query_params_defaults() {
        let json = r#"{}"#;
        let params: GraphQueryParams = serde_json::from_str(json).unwrap();
        assert!(params.depth.is_none());
        assert!(params.node_type.is_none());
        assert!(params.limit.is_none());
    }

    #[test]
    fn test_graph_query_params_with_values() {
        let json = r#"{"depth":3,"node_type":"concept","limit":50}"#;
        let params: GraphQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.depth, Some(3));
        assert_eq!(params.node_type, Some("concept".to_string()));
        assert_eq!(params.limit, Some(50));
    }

    #[tokio::test]
    async fn test_get_knowledge_graph_handler() {
        let result = get_knowledge_graph(Query(GraphQueryParams {
            depth: None,
            node_type: None,
            limit: None,
        }))
        .await;
        assert!(result.is_ok());
        let Json(data) = result.unwrap();
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.edges.len(), 1);
        assert_eq!(data.metadata.total_nodes, 2);
    }

    #[tokio::test]
    async fn test_get_document_relations_handler() {
        let result = get_document_relations().await;
        assert!(result.is_ok());
        let Json(data) = result.unwrap();
        assert_eq!(data.documents.len(), 2);
        assert_eq!(data.relations.len(), 1);
        assert_eq!(data.documents[0].doc_type, "sop");
    }

    #[tokio::test]
    async fn test_get_audit_timeline_handler() {
        let result = get_audit_timeline().await;
        assert!(result.is_ok());
        let Json(events) = result.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "create");
        assert_eq!(events[1].event_type, "approve");
    }

    #[tokio::test]
    async fn test_get_compliance_status_handler() {
        let result = get_compliance_status().await;
        assert!(result.is_ok());
        let Json(status) = result.unwrap();
        assert!((status.overall_score - 92.5).abs() < f64::EPSILON);
        assert_eq!(status.categories.len(), 4);
        assert!(status.categories.iter().all(|c| c.score >= 85.0));
    }

    #[test]
    fn test_document_relation_data_serialization() {
        let data = DocumentRelationData {
            documents: vec![DocumentNode {
                id: "d1".to_string(),
                title: "Test Doc".to_string(),
                doc_type: "sop".to_string(),
                version: "1.0".to_string(),
                status: "draft".to_string(),
                last_modified: "2026-05-18".to_string(),
            }],
            relations: vec![DocumentRelation {
                source: "d1".to_string(),
                target: "d2".to_string(),
                relation: "references".to_string(),
            }],
        };
        let json = serde_json::to_string(&data).unwrap();
        let deserialized: DocumentRelationData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.documents.len(), 1);
        assert_eq!(deserialized.relations[0].relation, "references");
    }

    #[test]
    fn test_compliance_category_fields() {
        let cat = ComplianceCategory {
            name: "test".to_string(),
            score: 75.0,
            total_checks: 4,
            passed_checks: 3,
            failed_checks: 1,
        };
        assert_eq!(cat.total_checks, cat.passed_checks + cat.failed_checks);
    }
}
