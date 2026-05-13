"""
Tests for KIAS components
"""
import pytest
import sys
from pathlib import Path

# Add src to path
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from scheduler.scheduler import Scheduler


@pytest.fixture
def sample_config():
    """Sample configuration for testing."""
    return {
        "scheduler": {
            "algorithm": "round-robin"
        }
    }


@pytest.fixture
def sample_nodes():
    """Sample nodes for testing."""
    return {
        "node1": {"cpu": "4", "memory": "8Gi"},
        "node2": {"cpu": "4", "memory": "8Gi"},
        "node3": {"cpu": "2", "memory": "4Gi"}
    }


class TestScheduler:
    """Test scheduler functionality."""
    
    def test_scheduler_initialization(self):
        """Test scheduler initialization."""
        scheduler = Scheduler(algorithm="round-robin")
        assert scheduler.algorithm.value == "round-robin"
    
    def test_register_node(self, sample_nodes):
        """Test node registration."""
        scheduler = Scheduler()
        
        for node_id, resources in sample_nodes.items():
            scheduler.register_node(node_id, resources)
        
        assert len(scheduler.nodes) == 3
    
    def test_unregister_node(self, sample_nodes):
        """Test node unregistration."""
        scheduler = Scheduler()
        
        for node_id, resources in sample_nodes.items():
            scheduler.register_node(node_id, resources)
        
        scheduler.unregister_node("node1")
        assert len(scheduler.nodes) == 2
        assert "node1" not in scheduler.nodes
    
    def test_round_robin_scheduling(self, sample_nodes):
        """Test round-robin scheduling."""
        scheduler = Scheduler(algorithm="round-robin")
        
        for node_id, resources in sample_nodes.items():
            scheduler.register_node(node_id, resources)
        
        # Schedule multiple agents
        scheduled_nodes = []
        for i in range(6):
            agent_spec = {"name": f"agent-{i}"}
            node_id = scheduler.schedule_agent(agent_spec)
            scheduled_nodes.append(node_id)
        
        # Should cycle through nodes
        assert scheduled_nodes[0] == "node1"
        assert scheduled_nodes[1] == "node2"
        assert scheduled_nodes[2] == "node3"
        assert scheduled_nodes[3] == "node1"  # Cycles back
    
    def test_least_loaded_scheduling(self, sample_nodes):
        """Test least-loaded scheduling."""
        scheduler = Scheduler(algorithm="least-loaded")
        
        for node_id, resources in sample_nodes.items():
            scheduler.register_node(node_id, resources)
        
        # Set loads
        scheduler.update_node_load("node1", 0.8)
        scheduler.update_node_load("node2", 0.3)
        scheduler.update_node_load("node3", 0.5)
        
        # Should schedule to least loaded node (node2)
        agent_spec = {"name": "test-agent"}
        node_id = scheduler.schedule_agent(agent_spec)
        assert node_id == "node2"
    
    def test_resource_aware_scheduling(self, sample_nodes):
        """Test resource-aware scheduling."""
        scheduler = Scheduler(algorithm="resource-aware")
        
        for node_id, resources in sample_nodes.items():
            scheduler.register_node(node_id, resources)
        
        # Schedule agent with high resource requirements
        agent_spec = {
            "name": "high-resource-agent",
            "resource_request": {"cpu": "2", "memory": "4Gi"}
        }
        
        node_id = scheduler.schedule_agent(agent_spec)
        assert node_id in ["node1", "node2"]  # node3 doesn't have enough resources
    
    def test_no_nodes_available(self):
        """Test scheduling when no nodes are available."""
        scheduler = Scheduler()
        
        agent_spec = {"name": "test-agent"}
        node_id = scheduler.schedule_agent(agent_spec)
        assert node_id is None
    
    def test_get_status(self, sample_nodes):
        """Test getting scheduler status."""
        scheduler = Scheduler()
        
        for node_id, resources in sample_nodes.items():
            scheduler.register_node(node_id, resources)
        
        status = scheduler.get_status()
        assert status["nodes"] == 3
        assert status["algorithm"] == "round-robin"


class TestNodeAgent:
    """Test node agent functionality."""
    
    def test_node_agent_creation(self):
        """Test node agent creation."""
        from node_agent.agent import NodeAgent
        
        config = {"node_agent": {"heartbeat_interval": 5}}
        agent = NodeAgent("test-node", config)
        
        assert agent.node_id == "test-node"
        assert agent.config == config


@pytest.mark.integration
class TestIntegration:
    """Integration tests."""
    
    def test_full_scheduling_flow(self, sample_nodes):
        """Test complete scheduling flow."""
        scheduler = Scheduler(algorithm="round-robin")
        
        # Register nodes
        for node_id, resources in sample_nodes.items():
            scheduler.register_node(node_id, resources)
        
        # Schedule multiple agents
        agents = [
            {"name": "agent-1", "resource_request": {"cpu": "0.5", "memory": "512Mi"}},
            {"name": "agent-2", "resource_request": {"cpu": "1", "memory": "1Gi"}},
            {"name": "agent-3", "resource_request": {"cpu": "0.25", "memory": "256Mi"}},
        ]
        
        scheduled = []
        for agent in agents:
            node_id = scheduler.schedule_agent(agent)
            scheduled.append((agent["name"], node_id))
        
        # All agents should be scheduled
        assert len(scheduled) == 3
        assert all(node_id is not None for _, node_id in scheduled)