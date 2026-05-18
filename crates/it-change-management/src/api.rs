//! # IT Change Management API Handlers
//!
//! Standalone API handlers - no dependency on api-server AppState.

pub mod types {
    use serde::{Deserialize, Serialize};
    use crate::{ChangeType, RiskLevel, Decision, ImpactAssessment};

    #[derive(Debug, Deserialize)]
    pub struct CreateChangeRequest {
        pub title: String,
        pub description: String,
        pub change_type: ChangeType,
        pub risk_level: RiskLevel,
        pub requester: String,
        pub rollback_plan: String,
        pub implementation_plan: String,
        pub impact_assessment: ImpactAssessment,
    }

    #[derive(Debug, Deserialize)]
    pub struct ApproveChangeRequest {
        pub approver_id: String,
        pub decision: Decision,
        pub signature: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct AddApproverRequest {
        pub user_id: String,
        pub name: String,
        pub role: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct RollbackRequest {
        pub rollback_by: String,
        pub reason: String,
    }

    #[derive(Debug, Serialize)]
    pub struct ApiResponse<T: Serialize> {
        pub success: bool,
        pub data: Option<T>,
        pub error: Option<String>,
    }

    impl<T: Serialize> ApiResponse<T> {
        pub fn ok(data: T) -> Self {
            Self { success: true, data: Some(data), error: None }
        }
        pub fn err(msg: String) -> Self {
            Self { success: false, data: None, error: Some(msg) }
        }
    }
}
