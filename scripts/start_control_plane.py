#!/usr/bin/env python3
"""
Script to start KIAS control plane.
"""
import uvicorn
import yaml
from pathlib import Path
import sys

# Add src to path
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))


def main():
    """Main function to start control plane."""
    # Load configuration
    config_path = Path(__file__).parent.parent / "config" / "config.yaml"
    
    if not config_path.exists():
        print(f"Config file not found: {config_path}")
        sys.exit(1)
    
    with open(config_path, 'r') as f:
        config = yaml.safe_load(f)
    
    # Get API configuration
    control_plane_config = config.get('control_plane', {})
    port = control_plane_config.get('api_port', 8080)
    log_level = control_plane_config.get('log_level', 'info')
    
    print(f"Starting KIAS Control Plane on port {port}")
    print(f"API documentation: http://localhost:{port}/docs")
    
    # Start server
    uvicorn.run(
        "control_plane.server:app",
        host="0.0.0.0",
        port=port,
        log_level=log_level,
        reload=True
    )


if __name__ == "__main__":
    main()