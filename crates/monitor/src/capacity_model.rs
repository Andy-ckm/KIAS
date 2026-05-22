python
#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
CapacityModel
=============
A flexible capacity‑planning engine that can be used for:

* **Single‑tenant** – one isolated workload that receives all the resources it asks for.
* **Multi‑tenant** – several workloads share a pool of resources; a *sharing factor*
  reduces the per‑tenant allocation.
* **Burst** – workloads that occasionally need more resources than their baseline,
  expressed via a *burst factor* and a *burst probability*.
* **Resource forecasting** – given a growth rate (e.g. 5 % per month) the model can
  estimate daily/weekly CPU, memory, storage and network consumption for a horizon.
* **Cost projection** – a pricing table (price per unit of each resource) is used to
  compute the total monthly or yearly spend for a given utilisation profile.

The module is deliberately written to be easy to extend (e.g. add a new resource
type) and to run in a CI pipeline (the tests are pure Python).

Usage (quick demo)
------------------
>>> from capacity_model import CapacityModel, TenantConfig, PricingTable, ResourceType
>>> tenants = [TenantConfig(name="app1", cpus=4, memory_gb=16, storage_gb=100,
...                         network_mbps=500, peak_factor=1.2, burst_factor=1.5)]
>>> pricing = PricingTable(cpu_price_per_core_hour=0.017,
...                        memory_price_per_gb_hour=0.007,
...                        storage_price_per_gb_hour=0.0001,
...                        network_price_per_mbps_hour=0.001)
>>> model = CapacityModel(tenants, pricing, sharing_factor=0.85)
>>> print(model.baseline_capacity())
"""

from __future__ import annotations

import calendar
import random
from dataclasses import dataclass, field
from datetime import date, timedelta
from enum import Enum, auto
from typing import Dict, List, Optional, Tuple, Union
from decimal import Decimal, ROUND_HALF_UP

# ----------------------------------------------------------------------
# Enumerations & Constants
# ----------------------------------------------------------------------


class ResourceType(Enum):
    """Supported resource types."""
    CPU = auto()
    MEMORY = auto()
    STORAGE = auto()
    NETWORK = auto()


# How many hours a billing month is assumed to have (average)
HOURS_PER_MONTH: float = 730.0  # 30.4167 days * 24


# ----------------------------------------------------------------------
# Data‑Transfer Objects (DTOs)
# ----------------------------------------------------------------------


@dataclass(frozen=True)
class TenantConfig:
    """
    Describes the resource demand of a single tenant.

    Attributes
    ----------
    name : str
        Human‑readable identifier for the tenant.
    cpus : float
        Desired number of CPU cores (or vCPUs) at baseline.
    memory_gb : float
        Desired RAM in GiB at baseline.
    storage_gb : float
        Desired persistent storage in GiB at baseline.
    network_mbps : float
        Desired network bandwidth in Mbps at baseline.
    peak_factor : float
        Multiplier applied to the baseline when the tenant experiences its *peak*
        (e.g. 1.2 → 20 % more than baseline).  Must be ≥ 1.0.
    burst_factor : float
        Multiplier applied to the baseline for occasional bursts.
        Must be ≥ 1.0.
    """

    name: str
    cpus: float = 0.0
    memory_gb: float = 0.0
    storage_gb: float = 0.0
    network_mbps: float = 0.0
    peak_factor: float = 1.0
    burst_factor: float = 1.0

    def __post_init__(self):
        if self.peak_factor < 1.0:
            raise ValueError("peak_factor must be >= 1.0")
        if self.burst_factor < 1.0:
            raise ValueError("burst_factor must be >= 1.0")


@dataclass
class PricingTable:
    """
    Contains the price of each resource per hour (or per unit‑hour).
    All prices are in USD unless otherwise noted.

    Attributes
    ----------
    cpu_price_per_core_hour : float
        Cost of one CPU core for one hour.
    memory_price_per_gb_hour : float
        Cost of 1 GiB of RAM for one hour.
    storage_price_per_gb_hour : float
        Cost of 1 GiB of persistent storage for one hour.
    network_price_per_mbps_hour : float
        Cost of 1 Mbps of network bandwidth for one hour.
    """

    cpu_price_per_core_hour: float = 0.0
    memory_price_per_gb_hour: float = 0.0
    storage_price_per_gb_hour: float = 0.0
    network_price_per_mbps_hour: float = 0.0

    def price_for(self, rtype: ResourceType) -> float:
        """Return the hourly price for the requested resource type."""
        mapping = {
            ResourceType.CPU: self.cpu_price_per_core_hour,
            ResourceType.MEMORY: self.memory_price_per_gb_hour,
            ResourceType.STORAGE: self.storage_price_per_gb_hour,
            ResourceType.NETWORK: self.network_price_per_mbps_hour,
        }
        return mapping[rtype]


@dataclass
class ResourceUsage:
    """
    Holds the usage of every resource type for a given time slice.
    """

    cpus: float = 0.0
    memory_gb: float = 0.0
    storage_gb: float = 0.0
    network_mbps: float = 0.0

    def __add__(self, other: ResourceUsage) -> ResourceUsage:
        return ResourceUsage(
            cpus=self.cpus + other.cpus,
            memory_gb=self.memory_gb + other.memory_gb,
            storage_gb=self.storage_gb + other.storage_gb,
            network_mbps=self.network_mbps + other.network_mbps,
        )

    def __mul__(self, factor: float) -> ResourceUsage:
        return ResourceUsage(
            cpus=self.cpus * factor,
            memory_gb=self.memory_gb * factor,
            storage_gb=self.storage_gb * factor,
            network_mbps=self.network_mbps * factor,
        )

    def __rmul__(self, factor: float) -> ResourceUsage:
        return self.__mul__(factor)

    def to_dict(self) -> Dict[str, float]:
        return dict(
            cpus=self.cpus,
            memory_gb=self.memory_gb,
            storage_gb=self.storage_gb,
            network_mbps=self.network_mbps,
        )

    def total_cost(self, pricing: PricingTable) -> Decimal:
        """Compute the hourly cost of this usage."""
        total = (
            self.cpus * pricing.cpu_price_per_core_hour
            + self.memory_gb * pricing.memory_price_per_gb_hour
            + self.storage_gb * pricing.storage_price_per_gb_hour
            + self.network_mbps * pricing.network_price_per_mbps_hour
        )
        return Decimal(str(total)).quantize(Decimal("0.0001"), rounding=ROUND_HALF_UP)


@dataclass
class ForecastEntry:
    """Single entry of a forecast timeline."""

    date: date
    usage: ResourceUsage
    cost: Decimal


# ----------------------------------------------------------------------
# Core CapacityModel
# ----------------------------------------------------------------------


class CapacityModel:
    """
    The main entry point for capacity planning.

    Parameters
    ----------
    tenants : List[TenantConfig]
        All workloads that should be accounted for.
    pricing : PricingTable
        Price list for each resource.
    sharing_factor : float, optional (default 1.0)
        For multi‑tenant workloads the allocated pool is reduced by this factor.
        Example: ``0.85`` means only 85 % of the raw sum is available for tenants.
    seed : int, optional
        Seed for the internal random generator (used in burst simulation).  If
        ``None`` the RNG is unseeded.
    """

    def __init__(
        self,
        tenants: List[TenantConfig],
        pricing: PricingTable,
        sharing_factor: float = 1.0,
        seed: Optional[int] = None,
    ):
        if not tenants:
            raise ValueError("At least one tenant must be provided")
        if not (0.0 < sharing_factor <= 1.0):
            raise ValueError("sharing_factor must be in (0, 1]")
        self._tenants = tenants
        self._pricing = pricing
        self._sharing_factor = sharing_factor
        self._rng = random.Random(seed)

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _aggregate(
        self, usage_list: List[ResourceUsage], factor: float = 1.0
    ) -> ResourceUsage:
        """Sum a list of ResourceUsage objects and optionally apply a factor."""
        total = ResourceUsage()
        for u in usage_list:
            total += u * factor
        return total

    def _tenant_to_usage(self, tenant: TenantConfig) -> ResourceUsage:
        """Convert a TenantConfig into a ResourceUsage at baseline."""
        return ResourceUsage(
            cpus=tenant.cpus,
            memory_gb=tenant.memory_gb,
            storage_gb=tenant.storage_gb,
            network_mbps=tenant.network_mbps,
        )

    def _apply_factor(
        self, usage: ResourceUsage, factor: float
    ) -> ResourceUsage:
        """Scale a ResourceUsage by a given factor."""
        return usage * factor

    # ------------------------------------------------------------------
    # Public API – baseline (steady‑state) capacity
    # ------------------------------------------------------------------

    def baseline_capacity(self) -> ResourceUsage:
        """
        Compute the total baseline capacity required for all tenants,
        optionally scaled by the multi‑tenant sharing factor.

        Returns
        -------
        ResourceUsage
            The sum of all baseline resource demands, after sharing factor is applied.
        """
        tenant_usages = [
            self._tenant_to_usage(t) for t in self._tenants
        ]
        raw = self._aggregate(tenant_usages)
        effective = self._apply_factor(raw, self._sharing_factor)
        return effective

    # ------------------------------------------------------------------
    # Peak capacity (worst‑case normal operation)
    # ------------------------------------------------------------------

    def peak_capacity(self) -> ResourceUsage:
        """
        Compute the capacity required when **all** tenants are at their peak.
        The peak factor is applied after the sharing factor.

        Returns
        -------
        ResourceUsage
            The total peak resource demand.
        """
        peak_usages = [
            self._apply_factor(self._tenant_to_usage(t), t.peak_factor)
            for t in self._tenants
        ]
        raw = self._aggregate(peak_usages)
        effective = self._apply_factor(raw, self._sharing_factor)
        return effective

    # ------------------------------------------------------------------
    # Burst handling
    # ------------------------------------------------------------------

    def simulate_burst(
        self, burst_probability: float = 0.1, burst_factor: Optional[float] = None
    ) -> ResourceUsage:
        """
        Randomly decide whether a burst occurs for each tenant and, if so,
        apply the tenant's ``burst_factor`` (or an optional override).

        Parameters
        ----------
        burst_probability : float
            Probability (0‑1) that a given tenant experiences a burst.
        burst_factor : float, optional
            If supplied, overrides the per‑tenant ``burst_factor`` for this
            simulation run.

        Returns
        -------
        ResourceUsage
            The simulated burst resource demand.
        """
        burst_usages = []
        for tenant in self._tenants:
            if self._rng.random() < burst_probability:
                # This tenant bursts – apply burst factor
                base = self._tenant_to_usage(tenant)
                factor = burst_factor if burst_factor is not None else tenant.burst_factor
                burst_usages.append(self._apply_factor(base, factor))
            else:
                # Normal baseline
                burst_usages.append(self._tenant_to_usage(tenant))

        raw = self._aggregate(burst_usages)
        effective = self._apply_factor(raw, self._sharing_factor)
        return effective

    def average_burst_capacity(
        self, trials: int = 10_000, burst_probability: float = 0.1
    ) -> ResourceUsage:
        """
        Run *trials* Monte‑Carlo simulations and return the average resource
        consumption when bursts are stochastic.

        Parameters
        ----------
        trials : int
            Number of Monte‑Carlo iterations.
        burst_probability : float
            Probability that a given tenant bursts in a single iteration.

        Returns
        -------
        ResourceUsage
            The **average** expected resource demand under burst conditions.
        """
        total_usage = ResourceUsage()
        for _ in range(trials):
            sim = self.simulate_burst(burst_probability=burst_probability)
            total_usage += sim
        # Compute the mean
        avg = total_usage * (1.0 / trials)
        return avg

    # ------------------------------------------------------------------
    # Resource forecasting
    # ------------------------------------------------------------------

    def forecast(
        self,
        horizon_days: int,
        daily_growth_rate: float = 0.0,
        start_date: Optional[date] = None,
        apply_sharing: bool = True,
    ) -> List[ForecastEntry]:
        """
        Generate a day‑by‑day forecast of resource usage and cost.

        Parameters
        ----------
        horizon_days : int
            Number of days into the future to forecast.
        daily_growth_rate : float
            Fractional growth applied **per day** to the baseline demand.
            Example: 0.005 → 0.5 % daily growth.
        start_date : date, optional
            First day of the forecast.  Defaults to today.
        apply_sharing : bool
            Whether to apply the sharing factor (True for multi‑tenant, False
            for pure single‑tenant simulations).

        Returns
        -------
        List[ForecastEntry]
            Ordered list of daily entries containing the date, usage and cost.
        """
        if horizon_days <= 0:
            raise ValueError("horizon_days must be positive")
        if start_date is None:
            start_date = date.today()

        # Baseline usage (without sharing factor) – we will apply it later if requested
        raw_baseline = self.baseline_capacity()
        forecast_entries: List[ForecastEntry] = []
        current_usage = raw_baseline  # start from today baseline

        for day in range(horizon_days):
            forecast_date = start_date + timedelta(days=day)
            # Apply daily growth
            current_usage = current_usage * (1.0 + daily_growth_rate)
            # Optionally apply sharing factor for multi‑tenant context
            effective_usage = (
                self._apply_factor(current_usage, self._sharing_factor)
                if apply_sharing
                else current_usage
            )
            cost = effective_usage.total_cost(self._pricing)
            forecast_entries.append(
                ForecastEntry(date=forecast_date, usage=effective_usage, cost=cost)
            )
        return forecast_entries

    # ------------------------------------------------------------------
    # Cost projection helpers
    # ------------------------------------------------------------------

    def cost_at_baseline(self) -> Decimal:
        """Return the hourly cost of the baseline capacity."""
        usage = self.baseline_capacity()
        return usage.total_cost(self._pricing)

    def cost_at_peak(self) -> Decimal:
        """Return the hourly cost of the peak capacity."""
        usage = self.peak_capacity()
        return usage.total_cost(self._pricing)

    def cost_of_burst(self, burst_probability: float = 0.1) -> Decimal:
        """
        Approximate the expected hourly cost when bursts occur with the given
        probability.  Uses the average_burst_capacity() method under the hood.
        """
        avg_usage = self.average_burst_capacity(burst_probability=burst_probability)
        return avg_usage.total_cost(self._pricing)

    # ------------------------------------------------------------------
    # Reporting utilities
    # ------------------------------------------------------------------

    def summary_report(self) -> str:
        """
        Generate a human‑readable report summarizing baseline, peak, average burst,
        and the first week of a forecast (if any tenants are defined).

        Returns
        -------
        str
            Multi‑line report string.
        """
        lines = ["=" * 60, "CAPACITY MODEL – SUMMARY REPORT", "=" * 60, ""]

        # Baseline & Peak
        baseline = self.baseline_capacity()
        peak = self.peak_capacity()
        lines.append("BASELINE CAPACITY")
        lines.append(
            f"  CPU cores       : {baseline.cpus:.2f}"
        )
        lines.append(
            f"  Memory (GiB)    : {baseline.memory_gb:.2f}"
        )
        lines.append(
            f"  Storage (GiB)    : {baseline.storage_gb:.2f}"
        )
        lines.append(
            f"  Network (Mbps)   : {baseline.network_mbps:.2f}"
        )
        lines.append(
            f"  Hourly cost ($)  : {self.cost_at_baseline():.4f}"
        )
        lines.append("")

        lines.append("PEAK CAPACITY (all tenants at peak)")
        lines.append(
            f"  CPU cores       : {peak.cpus:.2f}"
        )
        lines.append(
            f"  Memory (GiB)    : {peak.memory_gb:.2f}"
        )
        lines.append(
            f"  Storage (GiB)    : {peak.storage_gb:.2f}"
        )
        lines.append(
            f"  Network (Mbps)   : {peak.network_mbps:.2f}"
        )
        lines.append(
            f"  Hourly cost ($)  : {self.cost_at_peak():.4f}"
        )
        lines.append("")

        # Average burst (Monte‑Carlo)
        avg_burst = self.average_burst_capacity(trials=5_000, burst_probability=0.1)
        lines.append("AVERAGE BURST (Monte‑Carlo, 5 000 trials, 10 % burst prob.)")
        lines.append(
            f"  CPU cores       : {avg_burst.cpus:.2f}"
        )
        lines.append(
            f"  Memory (GiB)    : {avg_burst.memory_gb:.2f}"
        )
        lines.append(
            f"  Storage (GiB)    : {avg_burst.storage_gb:.2f}"
        )
        lines.append(
            f"  Network (Mbps)   : {avg_burst.network_mbps:.2f}"
        )
        lines.append(
            f"  Hourly cost ($)  : {self.cost_of_burst(burst_probability=0.1):.4f}"
        )
        lines.append("")

        # Short forecast (next 7 days)
        horizon = min(7, 365)
        forecast = self.forecast(
            horizon_days=horizon,
            daily_growth_rate=0.002,
        )
        lines.append(f"NEXT {horizon} DAYS FORECAST (2 % daily growth)")
        lines.append(
            "  Date        | CPU cores | Memory GiB | Storage GiB | Network Mbps | Hourly cost ($)"
        )
        lines.append("  " + "-" * 80)
        for entry in forecast:
            lines.append(
                f"  {entry.date.isoformat()} | "
                f"{entry.usage.cpus:9.2f} | "
                f"{entry.usage.memory_gb:10.2f} | "
                f"{entry.usage.storage_gb:10.2f} | "
                f"{entry.usage.network_mbps:12.2f} | "
                f"{entry.cost:.4f}"
            )
        lines.append("")
        lines.append("=" * 60)
        return "\n".join(lines)


# ----------------------------------------------------------------------
# Helper utilities (used by tests)
# ----------------------------------------------------------------------


def generate_random_tenants(
    count: int,
    seed: Optional[int] = None,
    min_cpus: float = 1.0,
    max_cpus: float = 16.0,
) -> List[TenantConfig]:
    """Create *count* random TenantConfig objects for testing."""
    rng = random.Random(seed)
    tenants = []
    for i in range(count):
        cpus = rng.uniform(min_cpus, max_cpus)
        memory = rng.uniform(2.0, 64.0)
        storage = rng.uniform(20.0, 500.0)
        network = rng.uniform(100.0, 1000.0)
        peak = rng.uniform(1.0, 1.5)
        burst = rng.uniform(1.0, 2.0)
        tenants.append(
            TenantConfig(
                name=f"tenant_{i+1}",
                cpus=cpus,
                memory_gb=memory,
                storage_gb=storage,
                network_mbps=network,
                peak_factor=peak,
                burst_factor=burst,
            )
        )
    return tenants


# ----------------------------------------------------------------------
# Command‑line entry point (optional)
# ----------------------------------------------------------------------


def _main():
    """Simple demo script when the module is executed directly."""
    tenants = generate_random_tenants(3, seed=42)
    pricing = PricingTable(
        cpu_price_per_core_hour=0.017,
        memory_price_per_gb_hour=0.007,
        storage_price_per_gb_hour=0.0001,
        network_price_per_mbps_hour=0.001,
    )
    model = CapacityModel(
        tenants=tenants,
        pricing=pricing,
        sharing_factor=0.90,
        seed=123,
    )
    print(model.summary_report())


if __name__ == "__main__":
    _main()