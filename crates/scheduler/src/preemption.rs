python
#!/usr/bin/env python3
"""
priority_preemption.py

A lightweight framework for priority‑based preemption of work‑units ("tasks").
Features:
* Priority‑based preemption – higher‑priority tasks can displace lower‑priority ones.
* Graceful eviction – tasks are given a chance to clean up before being killed.
* Resource reclaim – after eviction the released resources are made available again.
* Notification hub – callers can subscribe to preemption / eviction events.

The public API consists of:

* `TaskBase` – abstract base for any work unit that holds resources.
* `ResourceUsage` – a namedtuple describing how much of each resource a task uses.
* `ResourcePool` – tracks total available resources and the currently allocated set.
* `NotificationHub` – publish‑subscribe hub for events.
* `PreemptionPolicy` – abstract policy class; `PriorityBasedPolicy` is the default.
* `PreemptionManager` – the core orchestrator that runs tasks, evaluates preemption
  and evicts tasks when needed.

"""

from __future__ import annotations

import abc
import logging
import threading
import time
import uuid
from dataclasses import dataclass, field
from enum import Enum, auto
from typing import (
    Any,
    Callable,
    Dict,
    List,
    Mapping,
    MutableMapping,
    Optional,
    Set,
    Tuple,
)

# ----------------------------------------------------------------------
# Logging configuration
# ----------------------------------------------------------------------
logging.basicConfig(
    level=logging.DEBUG,
    format="[%(asctime)s] %(levelname)-8s %(name)s: %(message)s",
    datefmt="%H:%M:%S",
)
log = logging.getLogger("preempt")


# ----------------------------------------------------------------------
# Exceptions
# ----------------------------------------------------------------------
class PreemptionError(Exception):
    """Raised when a preemption operation cannot be performed."""
    pass


class ResourceExhaustedError(Exception):
    """Raised when there are not enough free resources to start a task."""
    pass


# ----------------------------------------------------------------------
# Data structures
# ----------------------------------------------------------------------
class ResourceType(Enum):
    """Kinds of resources that can be tracked."""
    CPU = auto()
    MEMORY = auto()
    DISK_IO = auto()
    NETWORK = auto()


@dataclass(frozen=True, order=True)
class ResourceUsage:
    """Immutable description of the amount of each resource a task needs."""
    cpu: int = 0
    memory: int = 0
    disk_io: int = 0
    network: int = 0

    def __add__(self, other: ResourceUsage) -> ResourceUsage:
        return ResourceUsage(
            cpu=self.cpu + other.cpu,
            memory=self.memory + other.memory,
            disk_io=self.disk_io + other.disk_io,
            network=self.network + other.network,
        )

    def __sub__(self, other: ResourceUsage) -> ResourceUsage:
        return ResourceUsage(
            cpu=max(0, self.cpu - other.cpu),
            memory=max(0, self.memory - other.memory),
            disk_io=max(0, self.disk_io - other.disk_io),
            network=max(0, self.network - other.network),
        )

    def fits_inside(self, capacity: ResourceUsage) -> bool:
        """Return True if this usage does not exceed the given capacity."""
        return (
            self.cpu <= capacity.cpu
            and self.memory <= capacity.memory
            and self.disk_io <= capacity.disk_io
            and self.network <= capacity.network
        )


# ----------------------------------------------------------------------
# Notification Hub
# ----------------------------------------------------------------------
class EventKind(Enum):
    TASK_STARTED = auto()
    TASK_STOPPED = auto()
    TASK_EVICTED = auto()
    RESOURCE_RECLAIMED = auto()
    PREEMPTION_TRIGGERED = auto()
    POLICY_CHANGED = auto()


@dataclass
class Event:
    kind: EventKind
    task_id: str
    data: Mapping[str, Any] = field(default_factory=dict)


Observer = Callable[[Event], None]


class NotificationHub:
    """Simple publish‑subscribe hub for internal events."""

    def __init__(self) -> None:
        self._subscribers: Dict[EventKind, List[Observer]] = {}
        self._lock = threading.RLock()

    def subscribe(self, kind: EventKind, observer: Observer) -> None:
        with self._lock:
            self._subscribers.setdefault(kind, []).append(observer)

    def unsubscribe(self, kind: EventKind, observer: Observer) -> None:
        with self._lock:
            if kind in self._subscribers:
                try:
                    self._subscribers[kind].remove(observer)
                except ValueError:
                    pass

    def notify(self, event: Event) -> None:
        with self._lock:
            observers = list(self._subscribers.get(event.kind, []))
        for obs in observers:
            try:
                obs(event)
            except Exception as exc:
                log.exception("Observer %s raised %s", obs, exc)


# ----------------------------------------------------------------------
# Task Base
# ----------------------------------------------------------------------
class TaskState(Enum):
    PENDING = auto()
    RUNNING = auto()
    STOPPING = auto()   # graceful shutdown in progress
    STOPPED = auto()
    EVICTED = auto()


class TaskBase(abc.ABC):
    """Abstract base for any work‑unit that can be preempted."""

    def __init__(
        self,
        task_id: Optional[str] = None,
        priority: int = 0,
        resource_usage: Optional[ResourceUsage] = None,
        hub: Optional[NotificationHub] = None,
    ) -> None:
        self.task_id = task_id or uuid.uuid4().hex
        self.priority = priority
        self.resource_usage = resource_usage or ResourceUsage()
        self._state = TaskState.PENDING
        self._state_lock = threading.RLock()
        self._stop_event = threading.Event()
        self._hub = hub

    # ------------------------------------------------------------------
    # Public state query / mutation
    # ------------------------------------------------------------------
    @property
    def state(self) -> TaskState:
        with self._state_lock:
            return self._state

    def _set_state(self, new_state: TaskState) -> None:
        with self._state_lock:
            self._state = new_state

    def request_stop(self) -> None:
        """Signal the task to stop gracefully."""
        self._stop_event.set()
        self._set_state(TaskState.STOPPING)
        log.debug("Task %s received stop request", self.task_id)

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------
    @abc.abstractmethod
    def do_work(self) -> None:
        """Perform the actual work – must respect self._stop_event."""
        raise NotImplementedError

    @abc.abstractmethod
    def do_cleanup(self) -> None:
        """Release any resources held by the task (file handles, memory, etc.)."""
        raise NotImplementedError

    def start(self) -> None:
        self._set_state(TaskState.RUNNING)
        self._hub and self._hub.notify(
            Event(EventKind.TASK_STARTED, self.task_id)
        )
        try:
            self.do_work()
        finally:
            self._finalize()

    def _finalize(self) -> None:
        self.do_cleanup()
        self._set_state(TaskState.STOPPED)
        self._hub and self._hub.notify(
            Event(EventKind.TASK_STOPPED, self.task_id)
        )

    # ------------------------------------------------------------------
    # Resource helpers (for testing)
    # ------------------------------------------------------------------
    def is_stop_requested(self) -> bool:
        return self._stop_event.is_set()

    def wait_until_stop_requested(self, timeout: Optional[float] = None) -> bool:
        return self._stop_event.wait(timeout=timeout)


# ----------------------------------------------------------------------
# Dummy concrete task (used for testing)
# ----------------------------------------------------------------------
class DummyTask(TaskBase):
    """A trivial task that just sleeps and optionally holds resources."""

    def __init__(
        self,
        *,
        work_duration: float = 10.0,
        **kwargs: Any,
    ) -> None:
        super().__init__(**kwargs)
        self.work_duration = work_duration

    def do_work(self) -> None:
        log.debug("DummyTask %s starting work (%.2fs)", self.task_id, self.work_duration)
        while not self.is_stop_requested():
            if self.wait_until_stop_requested(timeout=0.5):
                break
        log.debug("DummyTask %s work loop finished", self.task_id)

    def do_cleanup(self) -> None:
        log.debug("DummyTask %s cleaning up", self.task_id)


# ----------------------------------------------------------------------
# Resource Pool
# ----------------------------------------------------------------------
class ResourcePool:
    """Tracks total and currently free resources."""

    def __init__(self, capacity: ResourceUsage) -> None:
        self._capacity = capacity
        self._free = capacity
        self._lock = threading.RLock()

    @property
    def capacity(self) -> ResourceUsage:
        with self._lock:
            return self._capacity

    def allocate(self, usage: ResourceUsage) -> bool:
        with self._lock:
            if usage.fits_inside(self._free):
                self._free = self._free - usage
                log.debug("Allocated resources %s, free left: %s", usage, self._free)
                return True
            log.debug("Cannot allocate resources %s, free: %s", usage, self._free)
            return False

    def release(self, usage: ResourceUsage) -> None:
        with self._lock:
            self._free = self._free + usage
            log.debug("Released resources %s, free now: %s", usage, self._free)

    def total_free(self) -> ResourceUsage:
        with self._lock:
            return self._free


# ----------------------------------------------------------------------
# Preemption Policy
# ----------------------------------------------------------------------
class PreemptionPolicy(abc.ABC):
    """Abstract base for policies that decide which tasks to preempt."""

    @abc.abstractmethod
    def choose_tasks_to_evict(
        self,
        candidates: List[Tuple[TaskBase, ResourceUsage]],
        needed: ResourceUsage,
    ) -> List[TaskBase]:
        """Select a subset of *candidates* whose resources satisfy *needed*."""
        raise NotImplementedError


class PriorityBasedPolicy(PreemptionPolicy):
    """
    Always evict the lowest‑priority tasks first, breaking ties by task ID.
    """

    def choose_tasks_to_evict(
        self,
        candidates: List[Tuple[TaskBase, ResourceUsage]],
        needed: ResourceUsage,
    ) -> List[TaskBase]:
        # Sort by priority (ascending) then by task_id (ascending)
        sorted_candidates = sorted(
            candidates, key=lambda x: (x[0].priority, x[0].task_id)
        )
        selected: List[TaskBase] = []
        accumulated = ResourceUsage()
        for task, usage in sorted_candidates:
            selected.append(task)
            accumulated = accumulated + usage
            if accumulated.fits_inside(needed):
                break
        return selected


# ----------------------------------------------------------------------
# Preemption Manager
# ----------------------------------------------------------------------
class PreemptionManager:
    """
    Central coordinator that:

    * owns a `ResourcePool`,
    * stores all active tasks,
    * evaluates whether a new request can be satisfied,
    * preempts lower‑priority tasks if needed,
    * evicts selected tasks gracefully,
    * reclaims their resources,
    * notifies observers.
    """

    def __init__(
        self,
        pool_capacity: ResourceUsage,
        policy: Optional[PreemptionPolicy] = None,
        hub: Optional[NotificationHub] = None,
    ) -> None:
        self._pool = ResourcePool(pool_capacity)
        self._policy = policy or PriorityBasedPolicy()
        self._hub = hub or NotificationHub()
        self._tasks: MutableMapping[str, TaskBase] = {}
        self._task_resources: Dict[str, ResourceUsage] = {}
        self._lock = threading.RLock()
        self._worker_threads: Set[threading.Thread] = set()

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------
    def submit(self, task: TaskBase) -> None:
        """
        Attempt to start *task*.

        If the task's required resources are not available and its priority
        is higher than at least one running task, the manager will preempt
        lower‑priority tasks to make room.

        Raises:
            ResourceExhaustedError – if the task cannot be admitted and
                no lower‑priority task can be evicted.
        """
        with self._lock:
            if task.state != TaskState.PENDING:
                raise PreemptionError(f"Task {task.task_id} is not in PENDING state.")

            # Fast path – resources already available
            if self._pool.allocate(task.resource_usage):
                self._launch(task)
                return

            # Need to see if we can make room by evicting lower‑priority tasks
            if task.priority < self._lowest_running_priority():
                raise ResourceExhaustedError(
                    f"Cannot admit task {task.task_id} – insufficient resources."
                )

            self._preempt_and_launch(task)

    def cancel(self, task_id: str) -> None:
        """Request graceful cancellation of a running task."""
        with self._lock:
            task = self._tasks.get(task_id)
            if task is None:
                log.warning("Cancel request for unknown task %s", task_id)
                return
            self._request_graceful_stop(task)

    def active_tasks(self) -> List[TaskBase]:
        with self._lock:
            return list(self._tasks.values())

    # ------------------------------------------------------------------
    # Internals
    # ------------------------------------------------------------------
    def _lowest_running_priority(self) -> int:
        """Return the smallest priority among all RUNNING tasks (lower number = higher priority)."""
        running = [t for t in self._tasks.values() if t.state == TaskState.RUNNING]
        if not running:
            return float("inf")
        return min(t.priority for t in running)

    def _launch(self, task: TaskBase) -> None:
        self._tasks[task.task_id] = task
        self._task_resources[task.task_id] = task.resource_usage
        thr = threading.Thread(target=task.start, daemon=True)
        self._worker_threads.add(thr)
        thr.start()
        log.info("Task %s launched (priority %d)", task.task_id, task.priority)

    def _preempt_and_launch(self, incoming: TaskBase) -> None:
        """
        Choose victims, stop them gracefully, reclaim resources,
        then launch the incoming task.
        """
        needed = incoming.resource_usage - self._pool.total_free()
        victims = self._choose_victims(needed)
        if not victims:
            raise ResourceExhaustedError(
                f"Cannot make enough room for task {incoming.task_id}"
            )
        self._evict(victims)
        # Now resources should be available (they were returned by reclaim)
        if not self._pool.allocate(incoming.resource_usage):
            # This should never happen – but guard anyway
            raise ResourceExhaustedError("Resources still not available after eviction.")
        self._hub.notify(
            Event(
                EventKind.PREEMPTION_TRIGGERED,
                incoming.task_id,
                {"victims": [t.task_id for t in victims]},
            )
        )
        self._launch(incoming)

    def _choose_victims(self, needed: ResourceUsage) -> List[TaskBase]:
        """Return a list of tasks whose combined usage satisfies *needed*."""
        running_tasks = [
            (t, t.resource_usage)
            for t in self._tasks.values()
            if t.state == TaskState.RUNNING
        ]
        # Use the policy to decide which tasks to evict
        victims = self._policy.choose_tasks_to_evict(running_tasks, needed)
        return victims

    def _evict(self, victims: List[TaskBase]) -> None:
        """Signal each victim to stop gracefully and wait for them to finish."""
        log.info("Evicting %d tasks", len(victims))
        # Signal stop
        for task in victims:
            self._request_graceful_stop(task)

        # Wait for termination in a background thread (non‑blocking for the caller)
        def wait_and_reclaim(task: TaskBase) -> None:
            task.wait_until_stop_requested()
            # Give a short grace period for cleanup
            time.sleep(0.05)
            self._reclaim_task(task)

        for task in victims:
            t = threading.Thread(target=wait_and_reclaim, args=(task,), daemon=True)
            t.start()
            self._worker_threads.add(t)

    def _request_graceful_stop(self, task: TaskBase) -> None:
        task.request_stop()
        log.info("Graceful stop requested for task %s", task.task_id)
        self._hub.notify(Event(EventKind.TASK_EVICTED, task.task_id))

    def _reclaim_task(self, task: TaskBase) -> None:
        """Remove task from internal structures and release its resources."""
        with self._lock:
            if task.task_id in self._tasks:
                del self._tasks[task.task_id]
            usage = self._task_resources.pop(task.task_id, ResourceUsage())
            self._pool.release(usage)
        self._hub.notify(
            Event(
                EventKind.RESOURCE_RECLAIMED,
                task.task_id,
                {"released": usage},
            )
        )
        log.info("Resources reclaimed from task %s", task.task_id)


# ----------------------------------------------------------------------
# Optional helper – convenience wrapper to simulate a request
# ----------------------------------------------------------------------
def request_task(
    manager: PreemptionManager,
    priority: int,
    resource_usage: ResourceUsage,
    work_duration: float = 5.0,
) -> DummyTask:
    """Helper that creates a DummyTask, submits it to the manager and returns it."""
    task = DummyTask(
        priority=priority,
        resource_usage=resource_usage,
        work_duration=work_duration,
    )
    manager.submit(task)
    return task


# ----------------------------------------------------------------------
# __main__ – simple demo
# ----------------------------------------------------------------------
if __name__ == "__main__":
    # Very small resource pool for illustration
    capacity = ResourceUsage(cpu=4, memory=1024, disk_io=10, network=5)

    hub = NotificationHub()

    def printer(event: Event) -> None:
        print(f"📢 {event.kind.name} | task={event.task_id} | data={event.data}")

    for kind in EventKind:
        hub.subscribe(kind, printer)

    mgr = PreemptionManager(pool_capacity=capacity, hub=hub)

    print("Submitting low‑priority task (CPU 1, MEM 200) ...")
    t1 = request_task(mgr, priority=10, resource_usage=ResourceUsage(cpu=1, memory=200))

    print("Submitting medium‑priority task (CPU 2, MEM 300) ...")
    t2 = request_task(mgr, priority=5, resource_usage=ResourceUsage(cpu=2, memory=300))

    print("Submitting high‑priority task (CPU 4, MEM 800) – will preempt earlier tasks ...")
    t3 = request_task(mgr, priority=1, resource_usage=ResourceUsage(cpu=4, memory=800))

    # Let it run a bit
    time.sleep(2)
    print("Cancelling high‑priority task ...")
    mgr.cancel(t3.task_id)
    time.sleep(1)
    print("Active tasks:", [t.task_id for t in mgr.active_tasks()])
    print("Done.")