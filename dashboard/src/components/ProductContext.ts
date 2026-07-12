import { createContext, useContext } from 'react';

import type { ProductCapabilities } from '../types/capabilities';

export interface ProductContextValue {
  capabilities: ProductCapabilities;
  disconnect: () => void;
}

export const ProductContext = createContext<ProductContextValue | null>(null);

export function useProductContext(): ProductContextValue {
  const context = useContext(ProductContext);
  if (!context) {
    throw new Error('useProductContext must be used inside AuthGate');
  }
  return context;
}
