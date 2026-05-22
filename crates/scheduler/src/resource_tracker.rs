python
#!/usr/bin/env python3
"""
ResourceTracker: Tracks CPU, memory, and GPU resources per node.
Handles allocation, deallocation, and overcommit detection.
"""

from typing import Dict, Optional, Tuple, List
from dataclasses import dataclass, field
from enum import Enum
import threading
import time
from collections import defaultdict


class ResourceType(Enum):
    """Enum representing resource types."""
    CPU = "cpu"
    MEMORY = "memory"
    GPU = "gpu"


class OvercommitError(Exception):
    """Exception raised when resource allocation would overcommit available resources."""
    pass


class NodeNotFoundError(Exception):
    """Exception raised when a requested node is not found."""
    pass


class InvalidAllocationError(Exception):
    """Exception raised when an invalid allocation is attempted."""
    pass


@dataclass
class ResourceAllocation:
    """Represents a resource allocation for a job or task."""
    job_id: str
    node_id: str
    cpu: float
    memory: float
    gpu: float
    timestamp: float = field(default_factory=time.time)
    
    def to_dict(self) -> Dict:
        """Convert allocation to dictionary."""
        return {
            "job_id": self.job_id,
            "node_id": self.node_id,
            "cpu": self.cpu,
            "memory": self.memory,
            "gpu": self.gpu,
            "timestamp": self.timestamp
        }


@dataclass
class NodeResources:
    """Represents available resources on a node."""
    node_id: str
    total_cpu: float
    total_memory: float
    total_gpu: float
    available_cpu: float
    available_memory: float
    available_gpu: float
    
    def to_dict(self) -> Dict:
        """Convert node resources to dictionary."""
        return {
            "node_id": self.node_id,
            "total_cpu": self.total_cpu,
            "total_memory": self.total_memory,
            "total_gpu": self.total_gpu,
            "available_cpu": self.available_cpu,
            "available_memory": self.available_memory,
            "available_gpu": self.available_gpu
        }
    
    def get_utilization(self) -> Dict[str, float]:
        """Calculate resource utilization percentages."""
        return {
            "cpu_utilization": ((self.total_cpu - self.available_cpu) / self.total_cpu * 100) if self.total_cpu > 0 else 0,
            "memory_utilization": ((self.total_memory - self.available_memory) / self.total_memory * 100) if self.total_memory > 0 else 0,
            "gpu_utilization": ((self.total_gpu - self.available_gpu) / self.total_gpu * 100) if self.total_gpu > 0 else 0
        }


class ResourceTracker:
    """
    Central resource tracker for managing CPU, memory, and GPU resources across multiple nodes.
    
    Features:
    - Track available and allocated resources per node
    - Allocate and deallocate resources
    - Detect overcommit conditions
    - Thread-safe operations
    - Resource utilization monitoring
    """
    
    def __init__(self, enable_overcommit_detection: bool = True):
        """
        Initialize the ResourceTracker.
        
        Args:
            enable_overcommit_detection: If True, reject allocations that would overcommit resources
        """
        self._nodes: Dict[str, NodeResources] = {}
        self._allocations: Dict[str, List[ResourceAllocation]] = defaultdict(list)
        self._lock = threading.RLock()
        self._enable_overcommit_detection = enable_overcommit_detection
        self._allocation_history: List[ResourceAllocation] = []
    
    def add_node(self, node_id: str, cpu: float, memory: float, gpu: float) -> NodeResources:
        """
        Add a new node with specified resources.
        
        Args:
            node_id: Unique identifier for the node
            cpu: Total CPU cores available
            memory: Total memory (in GB) available
            gpu: Total GPU count available
            
        Returns:
            NodeResources object representing the new node
            
        Raises:
            ValueError: If resources are negative or node already exists
        """
        with self._lock:
            if node_id in self._nodes:
                raise ValueError(f"Node {node_id} already exists")
            
            if cpu < 0 or memory < 0 or gpu < 0:
                raise ValueError("Resources cannot be negative")
            
            node = NodeResources(
                node_id=node_id,
                total_cpu=cpu,
                total_memory=memory,
                total_gpu=gpu,
                available_cpu=cpu,
                available_memory=memory,
                available_gpu=gpu
            )
            self._nodes[node_id] = node
            return node
    
    def remove_node(self, node_id: str) -> bool:
        """
        Remove a node from the tracker.
        
        Args:
            node_id: ID of the node to remove
            
        Returns:
            True if node was removed, False if node wasn't found
        """
        with self._lock:
            if node_id not in self._nodes:
                return False
            
            # Check if node has active allocations
            if self._allocations[node_id]:
                raise InvalidAllocationError(f"Cannot remove node {node_id} with active allocations")
            
            del self._nodes[node_id]
            del self._allocations[node_id]
            return True
    
    def allocate(self, node_id: str, cpu: float, memory: float, gpu: float, 
                 job_id: Optional[str] = None) -> ResourceAllocation:
        """
        Allocate resources on a node.
        
        Args:
            node_id: Target node ID
            cpu: CPU cores to allocate
            memory: Memory (GB) to allocate
            gpu: GPU count to allocate
            job_id: Optional job identifier for tracking
            
        Returns:
            ResourceAllocation object
            
        Raises:
            NodeNotFoundError: If node doesn't exist
            OvercommitError: If allocation would overcommit resources
            ValueError: If allocation amounts are invalid
        """
        with self._lock:
            if node_id not in self._nodes:
                raise NodeNotFoundError(f"Node {node_id} not found")
            
            if cpu < 0 or memory < 0 or gpu < 0:
                raise ValueError("Allocation amounts cannot be negative")
            
            node = self._nodes[node_id]
            
            # Check for overcommit
            if self._enable_overcommit_detection:
                if not self._can_allocate(node_id, cpu, memory, gpu):
                    raise OvercommitError(
                        f"Allocation would overcommit node {node_id}. "
                        f"Requested: CPU={cpu}, Memory={memory}GB, GPU={gpu}. "
                        f"Available: CPU={node.available_cpu}, Memory={node.available_memory}GB, GPU={node.available_gpu}"
                    )
            
            # Perform allocation
            node.available_cpu -= cpu
            node.available_memory -= memory
            node.available_gpu -= gpu
            
            # Create allocation record
            allocation = ResourceAllocation(
                job_id=job_id or f"job_{len(self._allocation_history)}",
                node_id=node_id,
                cpu=cpu,
                memory=memory,
                gpu=gpu
            )
            
            self._allocations[node_id].append(allocation)
            self._allocation_history.append(allocation)
            
            return allocation
    
    def deallocate(self, node_id: str, cpu: float, memory: float, gpu: float,
                   job_id: Optional[str] = None) -> bool:
        """
        Deallocate resources from a node.
        
        Args:
            node_id: Target node ID
            cpu: CPU cores to deallocate
            memory: Memory (GB) to deallocate
            gpu: GPU count to deallocate
            job_id: Optional job ID to match specific allocation
            
        Returns:
            True if deallocation was successful
        """
        with self._lock:
            if node_id not in self._nodes:
                raise NodeNotFoundError(f"Node {node_id} not found")
            
            if cpu < 0 or memory < 0 or gpu < 0:
                raise ValueError("Deallocation amounts cannot be negative")
            
            node = self._nodes[node_id]
            
            # Validate deallocation won't exceed total resources
            new_available_cpu = node.available_cpu + cpu
            new_available_memory = node.available_memory + memory
            new_available_gpu = node.available_gpu + gpu
            
            if new_available_cpu > node.total_cpu:
                raise InvalidAllocationError(
                    f"Deallocation would exceed total CPU for node {node_id}"
                )
            if new_available_memory > node.total_memory:
                raise InvalidAllocationError(
                    f"Deallocation would exceed total memory for node {node_id}"
                )
            if new_available_gpu > node.total_gpu:
                raise InvalidAllocationError(
                    f"Deallocation would exceed total GPU for node {node_id}"
                )
            
            # Update available resources
            node.available_cpu = new_available_cpu
            node.available_memory = new_available_memory
            node.available_gpu = new_available_gpu
            
            # Remove allocation record if job_id provided, otherwise remove most recent
            if job_id and self._allocations[node_id]:
                # Find and remove matching allocation
                for i, alloc in enumerate(self._allocations[node_id]):
                    if alloc.job_id == job_id and alloc.cpu == cpu and alloc.memory == memory and alloc.gpu == gpu:
                        self._allocations[node_id].pop(i)
                        break
            elif self._allocations[node_id]:
                # Remove most recent allocation (LIFO)
                self._allocations[node_id].pop()
            
            return True
    
    def _can_allocate(self, node_id: str, cpu: float, memory: float, gpu: float) -> bool:
        """
        Check if allocation is possible without overcommitting.
        
        Args:
            node_id: Target node ID
            cpu: CPU cores to allocate
            memory: Memory (GB) to allocate
            gpu: GPU count to allocate
            
        Returns:
            True if allocation is possible, False otherwise
        """
        node = self._nodes[node_id]
        return (node.available_cpu >= cpu and 
                node.available_memory >= memory and 
                node.available_gpu >= gpu)
    
    def get_available_resources(self, node_id: str) -> Optional[NodeResources]:
        """
        Get available resources for a specific node.
        
        Args:
            node_id: Target node ID
            
        Returns:
            NodeResources object or None if node not found
        """
        with self._lock:
            return self._nodes.get(node_id)
    
    def get_all_nodes(self) -> Dict[str, NodeResources]:
        """
        Get all nodes and their resources.
        
        Returns:
            Dictionary of node_id to NodeResources
        """
        with self._lock:
            return self._nodes.copy()
    
    def get_allocations(self, node_id: Optional[str] = None) -> List[ResourceAllocation]:
        """
        Get current allocations.
        
        Args:
            node_id: Optional node ID to filter allocations
            
        Returns:
            List of ResourceAllocation objects
        """
        with self._lock:
            if node_id:
                return self._allocations.get(node_id, []).copy()
            return [alloc for allocs in self._allocations.values() for alloc in allocs]
    
    def get_total_utilization(self) -> Dict[str, float]:
        """
        Calculate overall resource utilization across all nodes.
        
        Returns:
            Dictionary with overall CPU, memory, and GPU utilization percentages
        """
        with self._lock:
            total_cpu = sum(n.total_cpu for n in self._nodes.values())
            total_memory = sum(n.total_memory for n in self._nodes.values())
            total_gpu = sum(n.total_gpu for n in self._nodes.values())
            
            used_cpu = sum(n.total_cpu - n.available_cpu for n in self._nodes.values())
            used_memory = sum(n.total_memory - n.available_memory for n in self._nodes.values())
            used_gpu = sum(n.total_gpu - n.available_gpu for n in self._nodes.values())
            
            return {
                "cpu_utilization": (used_cpu / total_cpu * 100) if total_cpu > 0 else 0,
                "memory_utilization": (used_memory / total_memory * 100) if total_memory > 0 else 0,
                "gpu_utilization": (used_gpu / total_gpu * 100) if total_gpu > 0 else 0,
                "total_nodes": len(self._nodes),
                "total_allocations": len(self._allocation_history)
            }
    
    def find_node_with_resources(self, cpu: float, memory: float, gpu: float) -> Optional[str]:
        """
        Find a node that has enough available resources.
        
        Args:
            cpu: Required CPU cores
            memory: Required memory (GB)
            gpu: Required GPU count
            
        Returns:
            Node ID that can accommodate the request, or None if no suitable node found
        """
        with self._lock:
            for node_id, node in self._nodes.items():
                if self._can_allocate(node_id, cpu, memory, gpu):
                    return node_id
            return None
    
    def reset(self):
        """Reset the tracker to initial state."""
        with self._lock:
            self._nodes.clear()
            self._allocations.clear()
            self._allocation_history.clear()
    
    def __repr__(self) -> str:
        with self._lock:
            return f"ResourceTracker(nodes={len(self._nodes)}, allocations={len(self._allocation_history)})"


# ============================================================================
# TEST SUITE
# ============================================================================

import unittest


class TestResourceTracker(unittest.TestCase):
    """Test cases for ResourceTracker."""
    
    def setUp(self):
        """Set up test fixtures."""
        self.tracker = ResourceTracker()
        
    def test_add_node(self):
        """Test adding a node to the tracker."""
        node = self.tracker.add_node("node1", cpu=16, memory=64, gpu=4)
        
        self.assertEqual(node.node_id, "node1")
        self.assertEqual(node.total_cpu, 16)
        self.assertEqual(node.total_memory, 64)
        self.assertEqual(node.total_gpu, 4)
        self.assertEqual(node.available_cpu, 16)
        self.assertEqual(node.available_memory, 64)
        self.assertEqual(node.available_gpu, 4)
    
    def test_allocate_resources(self):
        """Test allocating resources from a node."""
        self.tracker.add_node("node1", cpu=16, memory=64, gpu=4)
        
        allocation = self.tracker.allocate("node1", cpu=4, memory=16, gpu=1, job_id="job1")
        
        self.assertEqual(allocation.job_id, "job1")
        self.assertEqual(allocation.cpu, 4)
        self.assertEqual(allocation.memory, 16)
        self.assertEqual(allocation.gpu, 1)
        
        node = self.tracker.get_available_resources("node1")
        self.assertEqual(node.available_cpu, 12)
        self.assertEqual(node.available_memory, 48)
        self.assertEqual(node.available_gpu, 3)
    
    def test_deallocate_resources(self):
        """Test deallocating resources from a node."""
        self.tracker.add_node("node1", cpu=16, memory=64, gpu=4)
        self.tracker.allocate("node1", cpu=4, memory=16, gpu=1, job_id="job1")
        
        self.tracker.deallocate("node1", cpu=4, memory=16, gpu=1, job_id="job1")
        
        node = self.tracker.get_available_resources("node1")
        self.assertEqual(node.available_cpu, 16)
        self.assertEqual(node.available_memory, 64)
        self.assertEqual(node.available_gpu, 4)
    
    def test_overcommit_detection(self):
        """Test overcommit detection when allocating more than available."""
        self.tracker.add_node("node1", cpu=16, memory=64, gpu=4)
        
        # Allocate most resources
        self.tracker.allocate("node1", cpu=12, memory=48, gpu=3)
        
        # Try to allocate more than remaining (should fail with overcommit detection)
        with self.assertRaises(OvercommitError):
            self.tracker.allocate("node1", cpu=8, memory=32, gpu=2)
    
    def test_find_node_with_resources(self):
        """Test finding a node with sufficient resources."""
        self.tracker.add_node("node1", cpu=16, memory=64, gpu=4)
        self.tracker.add_node("node2", cpu=32, memory=128, gpu=8)
        self.tracker.add_node("node3", cpu=8, memory=32, gpu=2)
        
        # Allocate some resources from node2
        self.tracker.allocate("node2", cpu=16, memory=64, gpu=4)
        
        # Should find node1 or node3
        node_id = self.tracker.find_node_with_resources(cpu=8, memory=32, gpu=2)
        self.assertIn(node_id, ["node1", "node3"])
        
        # Should return None when no node can accommodate
        node_id = self.tracker.find_node_with_resources(cpu=32, memory=128, gpu=8)
        self.assertIsNone(node_id)
    
    def test_utilization_calculation(self):
        """Test resource utilization calculation."""
        self.tracker.add_node("node1", cpu=16, memory=64, gpu=4)
        self.tracker.add_node("node2", cpu=32, memory=128, gpu=8)
        
        self.tracker.allocate("node1", cpu=8, memory=32, gpu=2)
        self.tracker.allocate("node2", cpu=16, memory=64, gpu=4)
        
        utilization = self.tracker.get_total_utilization()
        
        self.assertEqual(utilization["total_nodes"], 2)
        # node1: 50% utilized, node2: 50% utilized -> overall 50%
        self.assertAlmostEqual(utilization["cpu_utilization"], 50.0, places=1)
        self.assertAlmostEqual(utilization["memory_utilization"], 50.0, places=1)
        self.assertAlmostEqual(utilization["gpu_utilization"], 50.0, places=1)


class TestResourceTrackerEdgeCases(unittest.TestCase):
    """Test edge cases and error handling."""
    
    def setUp(self):
        """Set up test fixtures."""
        self.tracker = ResourceTracker()
        
    def test_remove_node_with_allocations(self):
        """Test that removing a node with active allocations raises error."""
        self.tracker.add_node("node1", cpu=16, memory=64, gpu=4)
        self.tracker.allocate("node1", cpu=4, memory=16, gpu=1, job_id="job1")
        
        with self.assertRaises(InvalidAllocationError):
            self.tracker.remove_node("node1")
    
    def test_negative_resources_rejected(self):
        """Test that negative resource values are rejected."""
        with self.assertRaises(ValueError):
            self.tracker.add_node("node1", cpu=-4, memory=64, gpu=4)
    
    def test_allocation_of_zero_resources(self):
        """Test allocating zero resources is allowed."""
        self.tracker.add_node("node1", cpu=16, memory=64, gpu=4)
        allocation = self.tracker.allocate("node1", cpu=0, memory=0, gpu=0, job_id="job1")
        
        self.assertEqual(allocation.cpu, 0)
        node = self.tracker.get_available_resources("node1")
        self.assertEqual(node.available_cpu, 16)
    
    def test_nonexistent_node_allocation(self):
        """Test allocating from nonexistent node raises error."""
        with self.assertRaises(NodeNotFoundError):
            self.tracker.allocate("nonexistent", cpu=4, memory=16, gpu=1)
    
    def test_deallocation_exceeding_total(self):
        """Test deallocating more than total raises error."""
        self.tracker.add_node("node1", cpu=16, memory=64, gpu=4)
        
        with self.assertRaises(InvalidAllocationError):
            self.tracker.deallocate("node1", cpu=20, memory=64, gpu=4)


class TestResourceTrackerConcurrency(unittest.TestCase):
    """Test thread safety of ResourceTracker."""
    
    def test_concurrent_allocations(self):
        """Test concurrent allocation operations."""
        tracker = ResourceTracker()
        tracker.add_node("node1", cpu=100, memory=256, gpu=10)
        
        def allocate_resources():
            try:
                tracker.allocate("node1", cpu=5, memory=10, gpu=1)
            except (OvercommitError, InvalidAllocationError):
                pass  # Expected when resources depleted
        
        threads = [threading.Thread(target=allocate_resources) for _ in range(20)]
        
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        
        # Verify some allocations succeeded
        node = tracker.get_available_resources("node1")
        self.assertLess(node.available_cpu, 100)


if __name__ == "__main__":
    # Run tests
    unittest.main(verbosity=2)
    
    # Example usage
    print("\n" + "="*60)
    print("Example ResourceTracker Usage")
    print("="*60)
    
    tracker = ResourceTracker()
    
    # Add nodes
    tracker.add_node("gpu-cluster-1", cpu=64, memory=256, gpu=8)
    tracker.add_node("gpu-cluster-2", cpu=128, memory=512, gpu=16)
    
    # Allocate resources
    tracker.allocate("gpu-cluster-1", cpu=16, memory=64, gpu=2, job_id="training-job-1")
    tracker.allocate("gpu-cluster-1", cpu=8, memory=32, gpu=1, job_id="inference-job-1")
    tracker.allocate("gpu-cluster-2", cpu=32, memory=128, gpu=4, job_id="training-job-2")
    
    # Get utilization
    print("\nNode Status:")
    for node_id in ["gpu-cluster-1", "gpu-cluster-2"]:
        node = tracker.get_available_resources(node_id)
        util = node.get_utilization()
        print(f"  {node_id}:")
        print(f"    CPU: {util['cpu_utilization']:.1f}%")
        print(f"    Memory: {util['memory_utilization']:.1f}%")
        print(f"    GPU: {util['gpu_utilization']:.1f}%")
    
    # Find suitable node
    suitable = tracker.find_node_with_resources(cpu=16, memory=64, gpu=2)
    print(f"\nNode with resources for 16 CPU, 64GB RAM, 2 GPUs: {suitable}")
    
    # Total utilization
    print("\nOverall Utilization:")
    util = tracker.get_total_utilization()
    for key, value in util.items():
        print(f"  {key}: {value}")