use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::error::RegistryError;
use crate::types::*;

/// A2A Agent Registry — manages agent registration, discovery, and lifecycle.
///
/// Design inspired by EMQX 6.2's A2A Registry, extended with:
/// - Governance audit trail (every registration/discovery is logged)
/// - Schema validation on registration
/// - Multi-tenant org/unit isolation
#[derive(Debug)]
pub struct AgentRegistry {
    /// Registered agents: agent_id → AgentRegistration
    agents: Arc<RwLock<HashMap<String, AgentRegistration>>>,

    /// Discovery event broadcast channel
    event_tx: broadcast::Sender<DiscoveryEvent>,
}

impl AgentRegistry {
    /// Create a new registry with the given broadcast channel capacity
    pub fn new(event_capacity: usize) -> Self {
        let (event_tx, _) = broadcast::channel(event_capacity);
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    /// Subscribe to discovery events
    pub fn subscribe(&self) -> broadcast::Receiver<DiscoveryEvent> {
        self.event_tx.subscribe()
    }

    /// Register a new agent. Validates the Agent Card, then stores it.
    ///
    /// Returns the registration record on success.
    pub async fn register(&self, card: AgentCard) -> Result<AgentRegistration, RegistryError> {
        // Validate
        let validation = self.validate_card(&card);
        if !validation.valid {
            return Err(RegistryError::ValidationFailed(validation.errors));
        }

        let now = Utc::now();
        let registration = AgentRegistration {
            card: card.clone(),
            status: AgentStatus::Online,
            registered_at: now,
            last_seen: now,
            status_changed_at: now,
        };

        let mut agents = self.agents.write().await;
        if agents.contains_key(&card.agent_id) {
            return Err(RegistryError::AlreadyRegistered {
                agent_id: card.agent_id,
            });
        }

        agents.insert(card.agent_id.clone(), registration.clone());
        let result = registration.clone();
        info!(
            agent_id = %card.agent_id,
            org_id = %card.org_id,
            name = %card.name,
            "Agent registered"
        );

        // Emit discovery event
        let _ = self.event_tx.send(DiscoveryEvent {
            event_type: DiscoveryEventType::AgentRegistered,
            registration,
            timestamp: now,
        });

        Ok(result)
    }

    /// Deregister an agent
    pub async fn deregister(&self, agent_id: &str) -> Result<(), RegistryError> {
        let mut agents = self.agents.write().await;
        let registration = agents
            .remove(agent_id)
            .ok_or_else(|| RegistryError::NotFound {
                agent_id: agent_id.to_string(),
            })?;

        info!(agent_id = %agent_id, "Agent deregistered");

        let _ = self.event_tx.send(DiscoveryEvent {
            event_type: DiscoveryEventType::AgentDeregistered,
            registration,
            timestamp: Utc::now(),
        });

        Ok(())
    }

    /// Update agent status (online/offline/lwt)
    pub async fn update_status(
        &self,
        agent_id: &str,
        status: AgentStatus,
    ) -> Result<(), RegistryError> {
        let mut agents = self.agents.write().await;
        let registration = agents
            .get_mut(agent_id)
            .ok_or_else(|| RegistryError::NotFound {
                agent_id: agent_id.to_string(),
            })?;

        let old_status = registration.status.clone();
        if old_status == status {
            return Ok(());
        }

        let now = Utc::now();
        registration.status = status.clone();
        registration.status_changed_at = now;
        registration.last_seen = now;

        info!(
            agent_id = %agent_id,
            old_status = %old_status,
            new_status = %status,
            "Agent status changed"
        );

        let _ = self.event_tx.send(DiscoveryEvent {
            event_type: DiscoveryEventType::StatusChanged,
            registration: registration.clone(),
            timestamp: now,
        });

        Ok(())
    }

    /// Update an agent's card (capabilities, metadata, etc.)
    pub async fn update_card(&self, card: AgentCard) -> Result<(), RegistryError> {
        let validation = self.validate_card(&card);
        if !validation.valid {
            return Err(RegistryError::ValidationFailed(validation.errors));
        }

        let mut agents = self.agents.write().await;
        let registration =
            agents
                .get_mut(&card.agent_id)
                .ok_or_else(|| RegistryError::NotFound {
                    agent_id: card.agent_id.clone(),
                })?;

        registration.card = card;
        registration.last_seen = Utc::now();

        info!(agent_id = %registration.card.agent_id, "Agent card updated");

        let _ = self.event_tx.send(DiscoveryEvent {
            event_type: DiscoveryEventType::AgentUpdated,
            registration: registration.clone(),
            timestamp: Utc::now(),
        });

        Ok(())
    }

    /// Record a heartbeat (update last_seen timestamp)
    pub async fn heartbeat(&self, agent_id: &str) -> Result<(), RegistryError> {
        let mut agents = self.agents.write().await;
        let registration = agents
            .get_mut(agent_id)
            .ok_or_else(|| RegistryError::NotFound {
                agent_id: agent_id.to_string(),
            })?;

        registration.last_seen = Utc::now();
        Ok(())
    }

    /// Discover all registered agents
    pub async fn discover_all(&self) -> Vec<AgentRegistration> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Discover agents by organization
    pub async fn discover_by_org(&self, org_id: &str) -> Vec<AgentRegistration> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|r| r.card.org_id == org_id)
            .cloned()
            .collect()
    }

    /// Discover agents by capability
    pub async fn discover_by_capability(&self, capability_name: &str) -> Vec<AgentRegistration> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|r| {
                r.card
                    .capabilities
                    .iter()
                    .any(|c| c.name == capability_name)
            })
            .cloned()
            .collect()
    }

    /// Discover only online agents
    pub async fn discover_online(&self) -> Vec<AgentRegistration> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|r| r.status == AgentStatus::Online)
            .cloned()
            .collect()
    }

    /// Get a specific agent by ID
    pub async fn get_agent(&self, agent_id: &str) -> Result<AgentRegistration, RegistryError> {
        let agents = self.agents.read().await;
        agents
            .get(agent_id)
            .cloned()
            .ok_or_else(|| RegistryError::NotFound {
                agent_id: agent_id.to_string(),
            })
    }

    /// Count of registered agents
    pub async fn count(&self) -> usize {
        let agents = self.agents.read().await;
        agents.len()
    }

    /// Validate an Agent Card
    fn validate_card(&self, card: &AgentCard) -> ValidationResult {
        let mut errors = Vec::new();

        if card.agent_id.is_empty() {
            errors.push("agent_id is required".to_string());
        }
        if card.org_id.is_empty() {
            errors.push("org_id is required".to_string());
        }
        if card.unit_id.is_empty() {
            errors.push("unit_id is required".to_string());
        }
        if card.name.is_empty() {
            errors.push("name is required".to_string());
        }
        if card.version.is_empty() {
            errors.push("version is required".to_string());
        }

        // Validate capabilities have names
        for cap in &card.capabilities {
            if cap.name.is_empty() {
                errors.push("capability name is required".to_string());
            }
        }

        if errors.is_empty() {
            ValidationResult::ok()
        } else {
            ValidationResult::fail(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_card(agent_id: &str) -> AgentCard {
        AgentCard {
            agent_id: agent_id.to_string(),
            org_id: "test-org".to_string(),
            unit_id: "test-unit".to_string(),
            name: format!("Test Agent {agent_id}"),
            description: Some("A test agent".to_string()),
            capabilities: vec![AgentCapability {
                name: "text-generation".to_string(),
                description: Some("Generate text".to_string()),
                input_schema: None,
                output_schema: None,
            }],
            interaction_modes: vec![InteractionMode::RequestResponse],
            endpoint: Some("http://localhost:8080".to_string()),
            version: "1.0.0".to_string(),
            metadata: serde_json::json!({}),
        }
    }

    #[tokio::test]
    async fn test_register_and_discover() {
        let registry = AgentRegistry::new(100);
        let card = make_card("agent-1");

        let reg = registry.register(card).await.unwrap();
        assert_eq!(reg.status, AgentStatus::Online);

        let all = registry.discover_all().await;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].card.agent_id, "agent-1");
    }

    #[tokio::test]
    async fn test_register_duplicate_fails() {
        let registry = AgentRegistry::new(100);
        let card = make_card("agent-1");

        registry.register(card.clone()).await.unwrap();
        let result = registry.register(card).await;
        assert!(matches!(
            result,
            Err(RegistryError::AlreadyRegistered { .. })
        ));
    }

    #[tokio::test]
    async fn test_validation_empty_id() {
        let registry = AgentRegistry::new(100);
        let mut card = make_card("agent-1");
        card.agent_id = "".to_string();

        let result = registry.register(card).await;
        assert!(matches!(result, Err(RegistryError::ValidationFailed(_))));
    }

    #[tokio::test]
    async fn test_deregister() {
        let registry = AgentRegistry::new(100);
        let card = make_card("agent-1");

        registry.register(card).await.unwrap();
        assert_eq!(registry.count().await, 1);

        registry.deregister("agent-1").await.unwrap();
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_status_change() {
        let registry = AgentRegistry::new(100);
        let card = make_card("agent-1");

        registry.register(card).await.unwrap();

        registry
            .update_status("agent-1", AgentStatus::Offline)
            .await
            .unwrap();

        let agent = registry.get_agent("agent-1").await.unwrap();
        assert_eq!(agent.status, AgentStatus::Offline);
    }

    #[tokio::test]
    async fn test_discover_by_org() {
        let registry = AgentRegistry::new(100);

        let mut card1 = make_card("agent-1");
        card1.org_id = "org-a".to_string();
        let mut card2 = make_card("agent-2");
        card2.org_id = "org-b".to_string();
        let mut card3 = make_card("agent-3");
        card3.org_id = "org-a".to_string();

        registry.register(card1).await.unwrap();
        registry.register(card2).await.unwrap();
        registry.register(card3).await.unwrap();

        let org_a = registry.discover_by_org("org-a").await;
        assert_eq!(org_a.len(), 2);

        let org_b = registry.discover_by_org("org-b").await;
        assert_eq!(org_b.len(), 1);
    }

    #[tokio::test]
    async fn test_discover_by_capability() {
        let registry = AgentRegistry::new(100);

        let mut card1 = make_card("agent-1");
        card1.capabilities = vec![AgentCapability {
            name: "code-review".to_string(),
            description: None,
            input_schema: None,
            output_schema: None,
        }];
        let card2 = make_card("agent-2");

        registry.register(card1).await.unwrap();
        registry.register(card2).await.unwrap();

        let coders = registry.discover_by_capability("code-review").await;
        assert_eq!(coders.len(), 1);
        assert_eq!(coders[0].card.agent_id, "agent-1");

        let generators = registry.discover_by_capability("text-generation").await;
        assert_eq!(generators.len(), 1);
        assert_eq!(generators[0].card.agent_id, "agent-2");
    }

    #[tokio::test]
    async fn test_discovery_events() {
        let registry = AgentRegistry::new(100);
        let mut rx = registry.subscribe();

        let card = make_card("agent-1");
        registry.register(card).await.unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, DiscoveryEventType::AgentRegistered);
        assert_eq!(event.registration.card.agent_id, "agent-1");
    }

    #[tokio::test]
    async fn test_heartbeat() {
        let registry = AgentRegistry::new(100);
        let card = make_card("agent-1");

        registry.register(card).await.unwrap();

        let before = registry.get_agent("agent-1").await.unwrap().last_seen;

        // Small delay to ensure timestamp difference
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        registry.heartbeat("agent-1").await.unwrap();

        let after = registry.get_agent("agent-1").await.unwrap().last_seen;
        assert!(after >= before);
    }

    #[tokio::test]
    async fn test_update_card() {
        let registry = AgentRegistry::new(100);
        let mut card = make_card("agent-1");

        registry.register(card.clone()).await.unwrap();

        card.name = "Updated Agent".to_string();
        card.version = "2.0.0".to_string();
        registry.update_card(card).await.unwrap();

        let agent = registry.get_agent("agent-1").await.unwrap();
        assert_eq!(agent.card.name, "Updated Agent");
        assert_eq!(agent.card.version, "2.0.0");
    }
}
