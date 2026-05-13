#!/usr/bin/env python3
"""
Script to start KIAS node agent.
"""
import argparse
import sys
from pathlib import Path

# Add src to path
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))


def main():
    """Main function to start node agent."""
    parser = argparse.ArgumentParser(description="Start KIAS Node Agent")
    parser.add_argument("--node-id", required=True, help="Node ID")
    parser.add_argument("--config", default="config/config.yaml", help="Config file path")
    
    args = parser.parse_args()
    
    # Import and run node agent
    from node_agent.agent import main as agent_main
    
    # Override sys.argv for argparse
    sys.argv = [
        "agent.py",
        "--node-id", args.node_id,
        "--config", args.config
    ]
    
    agent_main()


if __name__ == "__main__":
    main()