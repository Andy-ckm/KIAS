"""
KIAS Scheduler - Schedules agents to nodes
"""
from typing import List, Dict, Any, Optional
import logging
from enum import Enum

logger = logging.getLogger(__name__)


class SchedulerAlgorithm(Enum):
    """Available scheduling algorithms."""
    ROUND_ROBIN = "round-robin"
    LEAST_LOADED = "least-loaded"
    RESOURCE_AWARE = "resource-aware"


class Scheduler:
    """Schedules agents to nodes based on resource availability."""
    
    def __init__(self, algorithm: str = "round-robin"):
        self.algorithm = SchedulerAlgorithm(algorithm)
        self.nodes = {}  # node_id -> node_info
        self.current_index = 0  # For round-robin
    
    def register_node(self, node_id: str, resources: Dict[str, Any]):
        """Register a node with the scheduler."""
        self.nodes[node_id] = {
            "resources": resources,
            "agents": [],
            "load": 0.0
        }
        logger.info(f"Registered node '{node_id}' with resources: {resources}")
    
    def unregister_node(self, node_id: str):
        """Unregister a node."""
        if node_id in self.nodes:
            del self.nodes[node_id]
            logger.info(f"Unregistered node '{node_id}'")
    
    def update_node_load(self, node_id: str, load: float):
        """Update node load information."""
        if node_id in self.nodes:
            self.nodes[node_id]["load"] = load
    
    def schedule_agent(self, agent_spec: Dict[str, Any]) -> Optional[str]:
        """Schedule an agent to a node."""
        if not self.nodes:
            logger.error("No nodes available for scheduling")
            return None
        
        if self.algorithm == SchedulerAlgorithm.ROUND_ROBIN:
            return self._schedule_round_robin(agent_spec)
        elif self.algorithm == SchedulerAlgorithm.LEAST_LOADED:
            return self._schedule_least_loaded(agent_spec)
        elif self.algorithm == SchedulerAlgorithm.RESOURCE_AWARE:
            return self._schedule_resource_aware(agent_spec)
        else:
            logger.error(f"Unknown algorithm: {self.algorithm}")
            return None
    
    def _schedule_round_robin(self, agent_spec: Dict[str, Any]) -> Optional[str]:
        """Schedule using round-robin algorithm."""
        node_ids = list(self.nodes.keys())
        node_id = node_ids[self.current_index % len(node_ids)]
        self.current_index += 1
        
        logger.info(f"Round-robin scheduled agent to node '{node_id}'")
        return node_id
    
    def _schedule_least_loaded(self, agent_spec: Dict[str, Any]) -> Optional[str]:
        """Schedule to least loaded node."""
        if not self.nodes:
            return None
        
        # Find node with lowest load
        min_load = float('inf')
        selected_node = None
        
        for node_id, node_info in self.nodes.items():
            if node_info["load"] < min_load:
                min_load = node_info["load"]
                selected_node = node_id
        
        logger.info(f"Least-loaded scheduled agent to node '{selected_node}'")
        return selected_node
    
    def _schedule_resource_aware(self, agent_spec: Dict[str, Any]) -> Optional[str]:
        """Schedule based on resource requirements."""
        # Get resource requirements from agent spec
        resource_request = agent_spec.get("resource_request", {})
        cpu_request = resource_request.get("cpu", "0.5")
        memory_request = resource_request.get("memory", "512Mi")
        
        # Convert to comparable values
        cpu_needed = float(cpu_request.replace("m", "")) / 1000 if "m" in cpu_request else float(cpu_request)
        memory_needed = self._parse_memory(memory_request)
        
        # Find node with enough resources
        for node_id, node_info in self.nodes.items():
            resources = node_info["resources"]
            cpu_available = float(resources.get("cpu", "0"))
            memory_available = self._parse_memory(resources.get("memory", "0Gi"))
            
            if cpu_available >= cpu_needed and memory_available >= memory_needed:
                logger.info(f"Resource-aware scheduled agent to node '{node_id}'")
                return node_id
        
        logger.warning("No node with sufficient resources found")
        return None
    
    def _parse_memory(self, memory_str: str) -> float:
        """Parse memory string to bytes."""
        if "Gi" in memory_str:
            return float(memory_str.replace("Gi", "")) * 1024 * 1024 * 1024
        elif "Mi" in memory_str:
            return float(memory_str.replace("Mi", "")) * 1024 * 1024
        elif "Ki" in memory_str:
            return float(memory_str.replace("Ki", "")) * 1024
        else:
            return float(memory_str)
    
    def get_status(self) -> Dict[str, Any]:
        """Get scheduler status."""
        return {
            "algorithm": self.algorithm.value,
            "nodes": len(self.nodes),
            "total_agents": sum(len(node["agents"]) for node in self.nodes.values())
        }