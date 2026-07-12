// Custom React hooks for KIAS API data fetching.

import { useState, useEffect, useCallback } from 'react';

interface UseApiState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
}

/**
 * Fetch data from the KIAS API and expose loading, error, and refetch state.
 *
 * Callers that capture changing values in `fetcher` must provide the same values
 * in `deps`. This mirrors React's effect dependency contract while keeping the
 * public refetch callback stable between dependency changes.
 */
export function useApi<T>(
  fetcher: () => Promise<T>,
  deps: unknown[] = []
): UseApiState<T> & { refetch: () => void } {
  const [state, setState] = useState<UseApiState<T>>({
    data: null,
    loading: true,
    error: null,
  });

  const fetchData = useCallback(async () => {
    setState(previous => ({ ...previous, loading: true, error: null }));
    try {
      const data = await fetcher();
      setState({ data, loading: false, error: null });
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : 'Unknown error';
      setState({ data: null, loading: false, error: message });
    }
    // The dependency list is intentionally caller-supplied; see the function contract above.
    // eslint-disable-next-line react-hooks/exhaustive-deps, react-hooks/use-memo
  }, deps);

  useEffect(() => {
    void fetchData();
  }, [fetchData]);

  return { ...state, refetch: fetchData };
}

/** Auto-refresh data at the configured interval. */
export function usePolling<T>(
  fetcher: () => Promise<T>,
  intervalMs: number = 5000,
  deps: unknown[] = []
): UseApiState<T> & { refetch: () => void } {
  const { data, loading, error, refetch } = useApi(fetcher, deps);

  useEffect(() => {
    const timer = window.setInterval(refetch, intervalMs);
    return () => window.clearInterval(timer);
  }, [refetch, intervalMs]);

  return { data, loading, error, refetch };
}