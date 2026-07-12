import { useEffect, useState, type FormEvent, type ReactNode } from 'react';

import {
  clearOperatorToken,
  getCapabilities,
  getOperatorToken,
  setOperatorToken,
} from '../api/client';
import type { ProductCapabilities } from '../types/capabilities';
import { ProductContext } from './ProductContext';

export default function AuthGate({ children }: { children: ReactNode }) {
  const [capabilities, setCapabilities] = useState<ProductCapabilities | null>(null);
  const [token, setToken] = useState(() => getOperatorToken() ?? '');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;

    getCapabilities()
      .then(response => {
        if (active) {
          setCapabilities(response);
          setError(null);
        }
      })
      .catch(cause => {
        if (active) {
          setCapabilities(null);
          setError(cause instanceof Error ? cause.message : 'Unable to connect to KIAS');
        }
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  async function verifyToken() {
    setLoading(true);
    setError(null);
    try {
      const response = await getCapabilities();
      setCapabilities(response);
    } catch (cause) {
      setCapabilities(null);
      setError(cause instanceof Error ? cause.message : 'Unable to connect to KIAS');
    } finally {
      setLoading(false);
    }
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setOperatorToken(token);
    await verifyToken();
  }

  function disconnect() {
    clearOperatorToken();
    setToken('');
    setCapabilities(null);
    setError('Operator token cleared');
  }

  if (loading) {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center text-slate-300">
        Verifying KIAS instance…
      </div>
    );
  }

  if (!capabilities) {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center p-6">
        <form
          onSubmit={submit}
          className="w-full max-w-md rounded-2xl border border-slate-700 bg-slate-900 p-7 shadow-xl"
        >
          <p className="text-xs font-semibold uppercase tracking-[0.2em] text-blue-400">
            KIAS operator access
          </p>
          <h1 className="mt-3 text-2xl font-bold text-white">Connect to the control plane</h1>
          <p className="mt-2 text-sm leading-6 text-slate-400">
            Paste a short-lived JWT or a local evaluation token. The value is kept only in this
            browser tab and is attached to authenticated API requests.
          </p>

          <label htmlFor="operator-token" className="mt-6 block text-sm font-medium text-slate-200">
            Operator token
          </label>
          <input
            id="operator-token"
            type="password"
            autoComplete="off"
            value={token}
            onChange={event => setToken(event.target.value)}
            className="mt-2 w-full rounded-lg border border-slate-600 bg-slate-950 px-3 py-2.5 text-sm text-white outline-none focus:border-blue-500"
            placeholder="eyJ… or local evaluation token"
          />

          {error && <p className="mt-3 text-sm text-amber-300">{error}</p>}

          <button
            type="submit"
            disabled={!token.trim()}
            className="mt-6 w-full rounded-lg bg-blue-600 px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Verify and continue
          </button>
        </form>
      </div>
    );
  }

  return (
    <ProductContext.Provider value={{ capabilities, disconnect }}>
      {children}
    </ProductContext.Provider>
  );
}
