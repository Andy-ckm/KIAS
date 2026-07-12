export type CapabilityTier = 'core' | 'extension' | 'labs';

export interface CapabilityDescriptor {
  id: string;
  label: string;
  tier: CapabilityTier;
  enabled: boolean;
  support: string;
}

export interface ProductCapabilities {
  product: string;
  version: string;
  profile: 'core' | 'core-with-extensions' | 'labs-enabled';
  contract: ['control', 'evidence', 'recovery'];
  capabilities: CapabilityDescriptor[];
}
