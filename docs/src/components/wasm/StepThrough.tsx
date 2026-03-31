import React, { useState, useEffect, useCallback } from 'react';
import BrowserOnly from '@docusaurus/BrowserOnly';
import { useFabulaWasm, parseResult } from '../../hooks/useFabulaWasm';
import DslEditor, { type ParseError } from './DslEditor';
import PresetSelector from './PresetSelector';
import { type Preset } from './presets';
import styles from './StepThrough.module.css';

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

interface StepThroughProps {
  defaultPattern?: string;
  defaultGraph?: string;
}

interface Step {
  timestamp: number;
  edges_added: { source: string; label: string; target: string }[];
  events: {
    type: string;
    pattern: string;
    match_id: number;
    stage_index?: number;
    bindings?: Record<string, string>;
    clause_label?: string;
  }[];
  partial_matches: {
    pattern: string;
    match_id: number;
    next_stage: number;
    state: string;
    bindings: Record<string, string>;
  }[];
}

function StepThroughInner({
  defaultPattern = DEFAULT_PATTERN,
  defaultGraph = DEFAULT_GRAPH,
}: StepThroughProps) {
  const { wasm, loading, error: wasmError } = useFabulaWasm();
  const [patternText, setPatternText] = useState(defaultPattern);
  const [graphText, setGraphText] = useState(defaultGraph);
  const [error, setError] = useState<ParseError | null>(null);
  const [steps, setSteps] = useState<Step[]>([]);
  const [currentStep, setCurrentStep] = useState(0);

  const evaluate = useCallback(() => {
    if (!wasm) return;

    const result = parseResult(wasm.evaluate_incremental(patternText, graphText));
    if (!result.ok) {
      setError(result.error);
      setSteps([]);
      return;
    }
    setError(null);
    setSteps(result.steps || []);
    setCurrentStep(0);
  }, [wasm, patternText, graphText]);

  useEffect(() => {
    if (!wasm) return;
    const timer = setTimeout(evaluate, 300);
    return () => clearTimeout(timer);
  }, [evaluate, wasm]);

  const handlePreset = useCallback((preset: Preset) => {
    setPatternText(preset.pattern);
    setGraphText(preset.graph);
  }, []);

  if (loading) return <div className={styles.loading}>Loading WASM engine...</div>;
  if (wasmError) return <div className={styles.error}>Failed to load WASM: {wasmError.message}</div>;

  const step = steps[currentStep];

  return (
    <div className={styles.container}>
      <PresetSelector onSelect={handlePreset} />
      <div className={styles.editors}>
        <DslEditor
          value={patternText}
          onChange={setPatternText}
          label="Pattern"
          height="160px"
        />
        <DslEditor
          value={graphText}
          onChange={setGraphText}
          label="Graph"
          error={error}
          height="160px"
        />
      </div>

      {steps.length > 0 && (
        <>
          <div className={styles.timeline}>
            <div className={styles.timelineLabel}>Timeline</div>
            <div className={styles.timelineSteps}>
              {steps.map((s, i) => (
                <button
                  key={i}
                  className={`${styles.timelineStep} ${i === currentStep ? styles.active : ''} ${i < currentStep ? styles.past : ''}`}
                  onClick={() => setCurrentStep(i)}
                  title={`t=${s.timestamp}: ${s.edges_added.length} edge(s)`}
                >
                  t={s.timestamp}
                </button>
              ))}
            </div>
            <div className={styles.timelineNav}>
              <button
                disabled={currentStep === 0}
                onClick={() => setCurrentStep(Math.max(0, currentStep - 1))}
              >
                Prev
              </button>
              <button
                disabled={currentStep >= steps.length - 1}
                onClick={() => setCurrentStep(Math.min(steps.length - 1, currentStep + 1))}
              >
                Next
              </button>
            </div>
          </div>

          {step && (
            <div className={styles.stepDetail}>
              <div className={styles.columns}>
                <div className={styles.column}>
                  <h4>Edges Added</h4>
                  {step.edges_added.map((e, i) => (
                    <div key={i} className={styles.edge}>
                      <code>{e.source}.{e.label} = {e.target}</code>
                    </div>
                  ))}
                </div>

                <div className={styles.column}>
                  <h4>Events</h4>
                  {step.events.length === 0 && (
                    <div className={styles.dim}>No events at this step</div>
                  )}
                  {step.events.map((ev, i) => (
                    <div key={i} className={`${styles.event} ${styles[ev.type]}`}>
                      <span className={styles.eventType}>{ev.type}</span>
                      <span className={styles.eventDetail}>
                        {ev.type === 'advanced' && `→ stage ${ev.stage_index}`}
                        {ev.type === 'completed' && 'Pattern matched!'}
                        {ev.type === 'negated' && `killed by ${ev.clause_label}`}
                      </span>
                    </div>
                  ))}
                </div>

                <div className={styles.column}>
                  <h4>Partial Matches</h4>
                  {step.partial_matches.map((pm, i) => (
                    <div key={i} className={`${styles.pm} ${styles['pm_' + pm.state]}`}>
                      <div className={styles.pmHeader}>
                        #{pm.match_id} — {pm.state}
                        {pm.state === 'active' && ` (next: stage ${pm.next_stage})`}
                      </div>
                      <div className={styles.pmBindings}>
                        {Object.entries(pm.bindings).map(([k, v]) => (
                          <span key={k} className={styles.binding}>?{k}={v}</span>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default function StepThrough(props: StepThroughProps) {
  return (
    <BrowserOnly fallback={<div>Loading step-through...</div>}>
      {() => <StepThroughInner {...props} />}
    </BrowserOnly>
  );
}
