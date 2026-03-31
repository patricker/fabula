import React, { useState, useEffect, useRef, useCallback } from 'react';
import BrowserOnly from '@docusaurus/BrowserOnly';
import { useFabulaWasm, parseResult } from '../../hooks/useFabulaWasm';
import DslEditor, { type ParseError } from './DslEditor';
import ResultsPanel from './ResultsPanel';
import PresetSelector from './PresetSelector';
import { type Preset } from './presets';
import styles from './PatternPlayground.module.css';

const DEFAULT_PATTERN = `pattern violation_of_hospitality {
  stage e1 {
    e1.eventType = "enterTown"
    e1.actor -> ?guest
  }
  stage e2 {
    e2.eventType = "showHospitality"
    e2.actor -> ?host
    e2.target -> ?guest
  }
  stage e3 {
    e3.eventType = "harm"
    e3.actor -> ?host
    e3.target -> ?guest
  }
  unless between e1 e3 {
    eMid.eventType = "leaveTown"
    eMid.actor -> ?guest
  }
}`;

const DEFAULT_GRAPH = `graph {
  @1 e1.eventType = "enterTown"
  @1 e1.actor -> alice
  @2 e2.eventType = "showHospitality"
  @2 e2.actor -> bob
  @2 e2.target -> alice
  @3 e3.eventType = "harm"
  @3 e3.actor -> bob
  @3 e3.target -> alice
  now = 10
}`;

interface PlaygroundProps {
  defaultPattern?: string;
  defaultGraph?: string;
  readonlyPattern?: boolean;
  compact?: boolean;
}

function PlaygroundInner({
  defaultPattern = DEFAULT_PATTERN,
  defaultGraph = DEFAULT_GRAPH,
  readonlyPattern = false,
  compact = false,
}: PlaygroundProps) {
  const { wasm, loading, error: wasmError } = useFabulaWasm();
  const [patternText, setPatternText] = useState(defaultPattern);
  const [graphText, setGraphText] = useState(defaultGraph);
  const [patternError, setPatternError] = useState<ParseError | null>(null);
  const [graphError, setGraphError] = useState<ParseError | null>(null);
  const [result, setResult] = useState<any>(null);
  const [resultMode, setResultMode] = useState<'batch' | 'gap'>('batch');
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const evaluate = useCallback(() => {
    if (!wasm) return;

    // Validate pattern
    const patResult = parseResult(wasm.parse_and_validate_pattern(patternText));
    if (!patResult.ok) {
      setPatternError(patResult.error);
      setResult(null);
      return;
    }
    setPatternError(null);

    // Validate graph
    const graphResult = parseResult(wasm.parse_and_validate_graph(graphText));
    if (!graphResult.ok) {
      setGraphError(graphResult.error);
      setResult(null);
      return;
    }
    setGraphError(null);

    // Batch evaluate
    const batchResult = parseResult(wasm.evaluate_batch(patternText, graphText));
    if (batchResult.ok && batchResult.matches && batchResult.matches.length > 0) {
      setResult(batchResult);
      setResultMode('batch');
    } else {
      // No matches — show gap analysis
      const gapResult = parseResult(wasm.why_not(patternText, graphText));
      setResult(gapResult);
      setResultMode('gap');
    }
  }, [wasm, patternText, graphText]);

  useEffect(() => {
    if (!wasm) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(evaluate, 300);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [evaluate, wasm]);

  const handlePreset = useCallback((preset: Preset) => {
    setPatternText(preset.pattern);
    setGraphText(preset.graph);
  }, []);

  if (loading) {
    return <div className={styles.loading}>Loading WASM engine...</div>;
  }
  if (wasmError) {
    return <div className={styles.wasmError}>Failed to load WASM: {wasmError.message}</div>;
  }

  const editorHeight = compact ? '180px' : '240px';

  return (
    <div className={styles.playground}>
      {!compact && <PresetSelector onSelect={handlePreset} />}
      <div className={styles.editors}>
        <div className={styles.editorPane}>
          <DslEditor
            value={patternText}
            onChange={setPatternText}
            label="Pattern"
            error={patternError}
            readonly={readonlyPattern}
            height={editorHeight}
          />
        </div>
        <div className={styles.editorPane}>
          <DslEditor
            value={graphText}
            onChange={setGraphText}
            label="Graph"
            error={graphError}
            height={editorHeight}
          />
        </div>
      </div>
      <div className={styles.results}>
        <div className={styles.resultsLabel}>
          {resultMode === 'batch' ? 'Matches' : 'Gap Analysis (why no match?)'}
        </div>
        <ResultsPanel result={result} mode={resultMode} />
      </div>
    </div>
  );
}

export default function PatternPlayground(props: PlaygroundProps) {
  return (
    <BrowserOnly fallback={<div>Loading playground...</div>}>
      {() => <PlaygroundInner {...props} />}
    </BrowserOnly>
  );
}
