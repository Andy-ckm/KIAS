//! # FSM Stage Constraint (SDOF-inspired)
//!
//! Intent-Stage Binding: every intent can only execute in a legal business stage.
//! Based on SDOF paper (2605.15204): "Taming the Alignment Tax in Multi-Agent Orchestration"
//!
//! Business process = Finite State Machine: (S, s₀, T, I, Λ)
//! - S = set of stages
//! - s₀ = initial stage
//! - T = transitions (stage → stage)
//! - I = set of intents
//! - Λ: I → 2^S (intent-to-legal-stages mapping)

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

// ── Errors ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageError {
    /// The stage does not exist in the FSM.
    StageNotFound(String),
    /// The transition is not allowed from the current stage.
    IllegalTransition { from: String, to: String },
    /// The intent is not allowed in the current stage.
    IntentNotAllowed { intent: String, stage: String },
    /// The FSM has no initial stage defined.
    NoInitialStage,
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StageNotFound(s) => write!(f, "Stage not found: {s}"),
            Self::IllegalTransition { from, to } => {
                write!(f, "Illegal transition from '{from}' to '{to}'")
            }
            Self::IntentNotAllowed { intent, stage } => {
                write!(f, "Intent '{intent}' not allowed in stage '{stage}'")
            }
            Self::NoInitialStage => write!(f, "No initial stage defined"),
        }
    }
}

impl std::error::Error for StageError {}

// ── Stage Definition ───────────────────────────────────────────────────

/// A single stage in the business process FSM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageDefinition {
    /// Stage identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Whether this is a terminal (final) stage.
    pub is_terminal: bool,
}

// ── Stage FSM ──────────────────────────────────────────────────────────

/// Finite State Machine for business process stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageFsm {
    /// All defined stages.
    stages: HashMap<String, StageDefinition>,
    /// Initial stage ID.
    initial_stage: Option<String>,
    /// Allowed transitions: from_stage → set of to_stages.
    transitions: HashMap<String, HashSet<String>>,
    /// Intent-to-legal-stages binding (Λ): intent → set of legal stages.
    intent_bindings: HashMap<String, HashSet<String>>,
    /// Current stage.
    current_stage: Option<String>,
}

impl StageFsm {
    /// Create a new empty FSM.
    pub fn new() -> Self {
        Self {
            stages: HashMap::new(),
            initial_stage: None,
            transitions: HashMap::new(),
            intent_bindings: HashMap::new(),
            current_stage: None,
        }
    }

    /// Add a stage definition.
    pub fn add_stage(&mut self, stage: StageDefinition) {
        if self.initial_stage.is_none() {
            self.initial_stage = Some(stage.id.clone());
        }
        self.stages.insert(stage.id.clone(), stage);
    }

    /// Set the initial stage.
    pub fn set_initial_stage(&mut self, stage_id: &str) {
        self.initial_stage = Some(stage_id.to_string());
    }

    /// Add a transition from one stage to another.
    pub fn add_transition(&mut self, from: &str, to: &str) {
        self.transitions
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
    }

    /// Bind an intent to legal stages (Λ mapping).
    pub fn bind_intent(&mut self, intent: &str, legal_stages: Vec<&str>) {
        self.intent_bindings.insert(
            intent.to_string(),
            legal_stages.into_iter().map(String::from).collect(),
        );
    }

    /// Initialize the FSM (set current stage to initial).
    pub fn initialize(&mut self) -> Result<(), StageError> {
        let initial = self
            .initial_stage
            .clone()
            .ok_or(StageError::NoInitialStage)?;
        self.current_stage = Some(initial);
        Ok(())
    }

    /// Get the current stage.
    pub fn current_stage(&self) -> Option<&str> {
        self.current_stage.as_deref()
    }

    /// Check if a transition is legal from the current stage.
    pub fn can_transition(&self, to: &str) -> bool {
        if let Some(current) = &self.current_stage {
            self.transitions
                .get(current)
                .map_or(false, |targets| targets.contains(to))
        } else {
            false
        }
    }

    /// Perform a stage transition.
    pub fn transition(&mut self, to: &str) -> Result<(), StageError> {
        let current = self
            .current_stage
            .clone()
            .ok_or(StageError::NoInitialStage)?;

        if !self.stages.contains_key(to) {
            return Err(StageError::StageNotFound(to.to_string()));
        }

        if !self.can_transition(to) {
            return Err(StageError::IllegalTransition {
                from: current,
                to: to.to_string(),
            });
        }

        self.current_stage = Some(to.to_string());
        Ok(())
    }

    /// Check if an intent is allowed in the current stage.
    pub fn is_intent_allowed(&self, intent: &str) -> bool {
        if let Some(current) = &self.current_stage {
            if let Some(legal_stages) = self.intent_bindings.get(intent) {
                return legal_stages.contains(current);
            }
        }
        // If no binding exists, allow by default
        true
    }

    /// Validate that an intent is allowed, returning error if not.
    pub fn validate_intent(&self, intent: &str) -> Result<(), StageError> {
        if let Some(current) = &self.current_stage {
            if !self.is_intent_allowed(intent) {
                return Err(StageError::IntentNotAllowed {
                    intent: intent.to_string(),
                    stage: current.clone(),
                });
            }
        }
        Ok(())
    }

    /// Get legal transitions from the current stage.
    pub fn legal_transitions(&self) -> Vec<String> {
        if let Some(current) = &self.current_stage {
            self.transitions
                .get(current)
                .map(|targets| targets.iter().cloned().collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Get all intents allowed in the current stage.
    pub fn allowed_intents(&self) -> Vec<String> {
        if let Some(current) = &self.current_stage {
            self.intent_bindings
                .iter()
                .filter(|(_, stages)| stages.contains(current))
                .map(|(intent, _)| intent.clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if the current stage is terminal.
    pub fn is_terminal(&self) -> bool {
        self.current_stage
            .as_ref()
            .and_then(|s| self.stages.get(s))
            .map_or(false, |s| s.is_terminal)
    }
}

impl Default for StageFsm {
    fn default() -> Self {
        Self::new()
    }
}

// ── Built-in Templates ─────────────────────────────────────────────────

/// Create a standard software delivery FSM.
pub fn software_delivery_fsm() -> StageFsm {
    let mut fsm = StageFsm::new();

    let stages = vec![
        StageDefinition { id: "init".to_string(), name: "Initialized".to_string(), is_terminal: false },
        StageDefinition { id: "src".to_string(), name: "Source Analysis".to_string(), is_terminal: false },
        StageDefinition { id: "int".to_string(), name: "Integration".to_string(), is_terminal: false },
        StageDefinition { id: "off".to_string(), name: "Offer/Proposal".to_string(), is_terminal: false },
        StageDefinition { id: "onb".to_string(), name: "Onboarding".to_string(), is_terminal: false },
        StageDefinition { id: "close".to_string(), name: "Closed".to_string(), is_terminal: true },
    ];
    for stage in stages {
        fsm.add_stage(stage);
    }

    // Transitions
    fsm.add_transition("init", "src");
    fsm.add_transition("src", "int");
    fsm.add_transition("int", "off");
    fsm.add_transition("off", "onb");
    fsm.add_transition("onb", "close");
    fsm.add_transition("off", "src"); // can loop back

    // Intent bindings
    fsm.bind_intent("analyze_code", vec!["src"]);
    fsm.bind_intent("run_tests", vec!["src", "int"]);
    fsm.bind_intent("deploy_staging", vec!["int"]);
    fsm.bind_intent("create_proposal", vec!["off"]);
    fsm.bind_intent("approve_proposal", vec!["off"]);
    fsm.bind_intent("onboard_user", vec!["onb"]);
    fsm.bind_intent("close_ticket", vec!["close"]);

    fsm
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fsm_initialization() {
        let mut fsm = software_delivery_fsm();
        fsm.initialize().unwrap();
        assert_eq!(fsm.current_stage(), Some("init"));
    }

    #[test]
    fn test_legal_transition() {
        let mut fsm = software_delivery_fsm();
        fsm.initialize().unwrap();
        assert!(fsm.can_transition("src"));
        fsm.transition("src").unwrap();
        assert_eq!(fsm.current_stage(), Some("src"));
    }

    #[test]
    fn test_illegal_transition() {
        let mut fsm = software_delivery_fsm();
        fsm.initialize().unwrap();
        // Cannot jump from init to close
        assert!(!fsm.can_transition("close"));
        let result = fsm.transition("close");
        assert!(matches!(
            result.unwrap_err(),
            StageError::IllegalTransition { .. }
        ));
    }

    #[test]
    fn test_intent_allowed_in_correct_stage() {
        let mut fsm = software_delivery_fsm();
        fsm.initialize().unwrap();
        fsm.transition("src").unwrap();

        assert!(fsm.is_intent_allowed("analyze_code"));
        assert!(fsm.is_intent_allowed("run_tests"));
        assert!(!fsm.is_intent_allowed("deploy_staging")); // not in src stage
        assert!(!fsm.is_intent_allowed("onboard_user"));
    }

    #[test]
    fn test_validate_intent_blocks() {
        let mut fsm = software_delivery_fsm();
        fsm.initialize().unwrap();

        // close_ticket not allowed in init stage
        let result = fsm.validate_intent("close_ticket");
        assert!(matches!(
            result.unwrap_err(),
            StageError::IntentNotAllowed { .. }
        ));
    }

    #[test]
    fn test_full_workflow() {
        let mut fsm = software_delivery_fsm();
        fsm.initialize().unwrap();

        // init → src
        fsm.transition("src").unwrap();
        assert!(fsm.validate_intent("analyze_code").is_ok());

        // src → int
        fsm.transition("int").unwrap();
        assert!(fsm.validate_intent("deploy_staging").is_ok());
        assert!(fsm.validate_intent("run_tests").is_ok());

        // int → off
        fsm.transition("off").unwrap();
        assert!(fsm.validate_intent("create_proposal").is_ok());

        // off → onb
        fsm.transition("onb").unwrap();
        assert!(fsm.validate_intent("onboard_user").is_ok());

        // onb → close
        fsm.transition("close").unwrap();
        assert!(fsm.is_terminal());
    }

    #[test]
    fn test_legal_transitions() {
        let mut fsm = software_delivery_fsm();
        fsm.initialize().unwrap();

        let transitions = fsm.legal_transitions();
        assert_eq!(transitions.len(), 1);
        assert!(transitions.contains(&"src".to_string()));
    }

    #[test]
    fn test_allowed_intents() {
        let mut fsm = software_delivery_fsm();
        fsm.initialize().unwrap();
        fsm.transition("src").unwrap();

        let intents = fsm.allowed_intents();
        assert!(intents.contains(&"analyze_code".to_string()));
        assert!(intents.contains(&"run_tests".to_string()));
    }

    #[test]
    fn test_stage_not_found() {
        let mut fsm = StageFsm::new();
        fsm.add_stage(StageDefinition {
            id: "a".to_string(),
            name: "A".to_string(),
            is_terminal: false,
        });
        fsm.set_initial_stage("a");
        fsm.initialize().unwrap();

        let result = fsm.transition("nonexistent");
        assert!(matches!(
            result.unwrap_err(),
            StageError::StageNotFound(_)
        ));
    }

    #[test]
    fn test_no_initial_stage() {
        let mut fsm = StageFsm::new();
        assert_eq!(fsm.initialize().unwrap_err(), StageError::NoInitialStage);
    }

    #[test]
    fn test_unknown_intent_allowed_by_default() {
        let mut fsm = software_delivery_fsm();
        fsm.initialize().unwrap();
        // Unknown intent with no binding → allowed
        assert!(fsm.is_intent_allowed("unknown_intent"));
    }

    #[test]
    fn test_error_display() {
        let err = StageError::IntentNotAllowed {
            intent: "deploy".to_string(),
            stage: "init".to_string(),
        };
        assert!(err.to_string().contains("deploy"));
        assert!(err.to_string().contains("init"));
    }

    #[test]
    fn test_default_fsm() {
        let fsm = StageFsm::default();
        assert!(fsm.stages.is_empty());
        assert!(fsm.current_stage().is_none());
    }
}
