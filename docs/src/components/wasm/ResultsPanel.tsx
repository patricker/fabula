import React from 'react';
import styles from './ResultsPanel.module.css';

interface ResultsPanelProps {
  result: any;
  mode: 'batch' | 'gap' | 'incremental';
}

export default function ResultsPanel({ result, mode }: ResultsPanelProps) {
  if (!result) {
    return <div className={styles.empty}>Edit the pattern or graph to see results</div>;
  }

  if (!result.ok) {
    return (
      <div className={styles.error}>
        <strong>Error:</strong> {result.error?.message || 'Unknown error'}
      </div>
    );
  }

  if (mode === 'batch') {
    return <BatchResults result={result} />;
  }
  if (mode === 'gap') {
    return <GapResults result={result} />;
  }
  return null;
}

function BatchResults({ result }: { result: any }) {
  const matches = result.matches || [];
  if (matches.length === 0) {
    return <div className={styles.noMatch}>No matches found. Try editing the graph.</div>;
  }

  return (
    <div className={styles.panel}>
      <div className={styles.matchCount}>
        {matches.length} match{matches.length !== 1 ? 'es' : ''}
      </div>
      {matches.map((m: any, i: number) => (
        <div key={i} className={styles.match}>
          <div className={styles.matchHeader}>Match #{i + 1}</div>
          <table className={styles.bindingTable}>
            <thead>
              <tr>
                <th>Variable</th>
                <th>Bound To</th>
              </tr>
            </thead>
            <tbody>
              {Object.entries(m.bindings || {}).map(([k, v]) => (
                <tr key={k}>
                  <td className={styles.varName}>?{k}</td>
                  <td>{String(v)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
    </div>
  );
}

function GapResults({ result }: { result: any }) {
  const analysis = result.analysis;
  if (!analysis) return null;

  return (
    <div className={styles.panel}>
      <div className={styles.gapHeader}>Gap Analysis: {analysis.pattern}</div>
      {analysis.stages.map((stage: any, i: number) => (
        <div key={i} className={styles.stage}>
          <div className={styles.stageHeader}>
            <span className={styles.stageAnchor}>{stage.anchor}</span>
            <StatusBadge status={stage.status} />
          </div>
          {stage.clauses.map((clause: any, j: number) => (
            <div key={j} className={`${styles.clause} ${clause.matched ? styles.matched : styles.unmatched}`}>
              <span className={styles.clauseIcon}>{clause.matched ? '\u2713' : '\u2717'}</span>
              <span className={styles.clauseDesc}>{clause.description}</span>
              {clause.reason && <span className={styles.clauseReason}>{clause.reason}</span>}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function StatusBadge({ status }: { status: string }) {
  const cls = status === 'matched'
    ? styles.statusMatched
    : status === 'unmatched'
      ? styles.statusUnmatched
      : styles.statusPartial;

  return <span className={`${styles.statusBadge} ${cls}`}>{status}</span>;
}
