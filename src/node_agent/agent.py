"""
KIAS Node Agent
"""
import argparse
import logging
import time
import psutil
from typing import Dict, Any
import yaml
from pathlib import Path

logger = logging.getLogger(__name__)


class NodeAgent:
    """Agent running on each node to manage local agents."""
    
    def __init__(self, node_id: str, config: Dict[str, Any]):
        self.node_id = node_id
        self.config = config
        self.agents = {}  # agent_name -> agent_info
        
    def get_system_resources(self) -> Dict[str, Any]:
        """Get current system resource usage."""
        cpu_percent = psutil.cpu_percent(interval=1)
        memory = psutil.virtual_memory()
        
        return {
            "cpu_percent": cpu_percent,
            "memory_total_gb": round(memory.total / (1024**3), 2),
            "memory_used_gb": round(memory.used / (1024**3), 2),
            "memory_percent": memory.percent
        }
    
    def start_agent(self, agent_name: str, agent_spec: Dict[str, Any]) -> bool:
        """Start an agent on this node."""
        logger.info(f"Starting agent '{agent_name}' on node '{self.node_id}'")
        
        # TODO: Implement actual agent starting
        # This would involve:
        # 1. Pulling the container image
        # 2. Creating the container
        # 3. Starting the container
        # 4. Monitoring the container
        
        self.agents[agent_name] = {
            "spec": agent_spec,
            "status": "Running",
            "start_time": time.time(),
            "restart_count": 0
        }
        
        return True
    
    def stop_agent(self, agent_name: str) -> bool:
        """Stop an agent on this node."""
        if agent_name not in self.agents:
            logger.error(f"Agent '{agent_name}' not found")
            return False
        
        logger.info(f"Stopping agent '{agent_name}' on node '{self.node_id}'")
        
        # TODO: Implement actual agent stopping
        
        del self.agents[agent_name]
        return True
    
    def get_agent_status(self, agent_name: str) -> Dict[str, Any]:
        """Get status of an agent."""
        if agent_name not in self.agents:
            return None
        
        return self.agents[agent_name]
    
    def list_agents(self) -> Dict[str, Any]:
        """List all agents on this node."""
        return self.agents
    
    def send_heartbeat(self):
        """Send heartbeat to control plane."""
        # TODO: Implement heartbeat sending
        resources = self.get_system_resources()
        logger.debug(f"Heartbeat from node '{self.node_id}': {resources}")
    
    def run(self):
        """Main loop for node agent."""
        logger.info(f"Starting node agent '{self.node_id}'")
        
        heartbeat_interval = self.config.get("node_agent", {}).get("heartbeat_interval", 5)
        
        while True:
            try:
                self.send_heartbeat()
                time.sleep(heartbeat_interval)
            except KeyboardInterrupt:
                logger.info("Node agent shutting down...")
                break
            except Exception as e:
                logger.error(f"Error in node agent: {e}")
                time.sleep(1)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description="KIAS Node Agent")
    parser.add_argument("--node-id", required=True, help="Node ID")
    parser.add_argument("--config", default="config/config.yaml", help="Config file path")
    
    args = parser.parse_args()
    
    # Configure logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    # Load configuration
    config_path = Path(args.config)
    if config_path.exists():
        with open(config_path, 'r') as f:
            config = yaml.safe_load(f)
    else:
        config = {}
    
    # Create and run node agent
    agent = NodeAgent(args.node_id, config)
    agent.run()


if __name__ == "__main__":
    main()