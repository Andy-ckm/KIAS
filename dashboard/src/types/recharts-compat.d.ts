import type { ComponentType } from 'react';

// Re-export the installed Recharts declarations while narrowing the compatibility
// escape hatch to Tooltip only. Recharts 3 accepts undefined/general ValueType
// inputs, while existing KIAS chart formatters operate on normalized numeric data.
export * from '../../node_modules/recharts/types/index';

export const Tooltip: ComponentType<Record<string, unknown>>;
