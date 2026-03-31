import { useState, useEffect, useRef } from 'react';
import useBaseUrl from '@docusaurus/useBaseUrl';

export interface FabulaWasm {
  parse_and_validate_pattern: (dsl: string) => string;
  parse_and_validate_graph: (dsl: string) => string;
  evaluate_batch: (pattern: string, graph: string) => string;
  evaluate_incremental: (pattern: string, graph: string) => string;
  why_not: (pattern: string, graph: string) => string;
  allen_relation: (a_start: number, a_end: number, b_start: number, b_end: number) => string; // f64 on WASM side
}

export interface WasmState {
  wasm: FabulaWasm | null;
  loading: boolean;
  error: Error | null;
}

export function useFabulaWasm(): WasmState {
  const [wasm, setWasm] = useState<FabulaWasm | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const initRef = useRef(false);
  const baseUrl = useBaseUrl('/');

  useEffect(() => {
    if (initRef.current) return;
    initRef.current = true;

    (async () => {
      try {
        const wasmUrl = `${baseUrl}wasm/pkg/fabula_wasm.js`;
        // Dynamic import to avoid SSR issues
        const importFn = new Function('url', 'return import(url)');
        const wasmModule = await importFn(wasmUrl);
        // wasm-bindgen init — loads the .wasm file
        await wasmModule.default();

        // Wrap each function to parse JSON result strings
        const wrap = (fn: ((...args: any[]) => string)) => {
          return (...args: any[]) => {
            const result = fn(...args);
            return result;
          };
        };

        setWasm({
          parse_and_validate_pattern: wrap(wasmModule.parse_and_validate_pattern),
          parse_and_validate_graph: wrap(wasmModule.parse_and_validate_graph),
          evaluate_batch: wrap(wasmModule.evaluate_batch),
          evaluate_incremental: wrap(wasmModule.evaluate_incremental),
          why_not: wrap(wasmModule.why_not),
          allen_relation: wrap(wasmModule.allen_relation),
        });
        setLoading(false);
      } catch (e) {
        setError(e instanceof Error ? e : new Error(String(e)));
        setLoading(false);
      }
    })();
  }, [baseUrl]);

  return { wasm, loading, error };
}

/** Parse the JSON string returned by WASM functions. */
export function parseResult(result: string): any {
  return JSON.parse(result);
}
