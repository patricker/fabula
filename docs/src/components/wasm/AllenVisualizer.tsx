import React, { useState, useCallback, useRef, useEffect } from 'react';
import BrowserOnly from '@docusaurus/BrowserOnly';
import { useFabulaWasm, parseResult } from '../../hooks/useFabulaWasm';
import styles from './AllenVisualizer.module.css';

interface AllenVisualizerProps {
  presets?: boolean;
}

const PRESETS: Record<string, { a: [number, number]; b: [number, number] }> = {
  Before:       { a: [10, 30], b: [40, 60] },
  After:        { a: [40, 60], b: [10, 30] },
  Meets:        { a: [10, 30], b: [30, 50] },
  MetBy:        { a: [30, 50], b: [10, 30] },
  Overlaps:     { a: [10, 40], b: [25, 55] },
  OverlappedBy: { a: [25, 55], b: [10, 40] },
  During:       { a: [25, 40], b: [10, 55] },
  Contains:     { a: [10, 55], b: [25, 40] },
  Starts:       { a: [10, 30], b: [10, 50] },
  StartedBy:    { a: [10, 50], b: [10, 30] },
  Finishes:     { a: [30, 50], b: [10, 50] },
  FinishedBy:   { a: [10, 50], b: [30, 50] },
  Equals:       { a: [15, 45], b: [15, 45] },
};

const AXIS_MIN = 0;
const AXIS_MAX = 70;
const SVG_WIDTH = 500;
const SVG_HEIGHT = 100;
const PAD = 40;
const BAR_H = 16;
const A_Y = 28;
const B_Y = 60;

function toX(val: number): number {
  return PAD + ((val - AXIS_MIN) / (AXIS_MAX - AXIS_MIN)) * (SVG_WIDTH - 2 * PAD);
}

function fromX(x: number): number {
  const val = AXIS_MIN + ((x - PAD) / (SVG_WIDTH - 2 * PAD)) * (AXIS_MAX - AXIS_MIN);
  return Math.round(Math.max(AXIS_MIN, Math.min(AXIS_MAX, val)));
}

function AllenVisualizerInner({ presets = false }: AllenVisualizerProps) {
  const { wasm, loading } = useFabulaWasm();
  const [aStart, setAStart] = useState(10);
  const [aEnd, setAEnd] = useState(40);
  const [bStart, setBStart] = useState(25);
  const [bEnd, setBEnd] = useState(55);
  const [relation, setRelation] = useState<string | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const dragRef = useRef<{
    handle: 'aStart' | 'aEnd' | 'bStart' | 'bEnd';
  } | null>(null);

  const computeRelation = useCallback(() => {
    if (!wasm) return;
    const result = parseResult(wasm.allen_relation(aStart, aEnd, bStart, bEnd));
    setRelation(result.ok ? result.relation : null);
  }, [wasm, aStart, aEnd, bStart, bEnd]);

  useEffect(() => {
    computeRelation();
  }, [computeRelation]);

  const handleMouseDown = useCallback(
    (handle: 'aStart' | 'aEnd' | 'bStart' | 'bEnd') => (e: React.MouseEvent) => {
      e.preventDefault();
      dragRef.current = { handle };
    },
    []
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (!dragRef.current || !svgRef.current) return;
      const rect = svgRef.current.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const val = fromX(x);

      switch (dragRef.current.handle) {
        case 'aStart':
          if (val < aEnd) setAStart(val);
          break;
        case 'aEnd':
          if (val > aStart) setAEnd(val);
          break;
        case 'bStart':
          if (val < bEnd) setBStart(val);
          break;
        case 'bEnd':
          if (val > bStart) setBEnd(val);
          break;
      }
    },
    [aStart, aEnd, bStart, bEnd]
  );

  const handleMouseUp = useCallback(() => {
    dragRef.current = null;
  }, []);

  const applyPreset = (name: string) => {
    const p = PRESETS[name];
    setAStart(p.a[0]);
    setAEnd(p.a[1]);
    setBStart(p.b[0]);
    setBEnd(p.b[1]);
  };

  if (loading) return <div className={styles.loading}>Loading...</div>;

  return (
    <div className={styles.container}>
      {presets && (
        <div className={styles.presets}>
          <label>Presets:</label>
          <select onChange={(e) => applyPreset(e.target.value)} defaultValue="">
            <option value="" disabled>Select a relation...</option>
            {Object.keys(PRESETS).map((name) => (
              <option key={name} value={name}>{name}</option>
            ))}
          </select>
        </div>
      )}
      <svg
        ref={svgRef}
        viewBox={`0 0 ${SVG_WIDTH} ${SVG_HEIGHT}`}
        className={styles.svg}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
      >
        {/* Axis */}
        <line x1={PAD} y1={SVG_HEIGHT - 10} x2={SVG_WIDTH - PAD} y2={SVG_HEIGHT - 10} stroke="var(--ifm-color-emphasis-300)" strokeWidth="1" />
        {[0, 10, 20, 30, 40, 50, 60, 70].map((v) => (
          <g key={v}>
            <line x1={toX(v)} y1={SVG_HEIGHT - 14} x2={toX(v)} y2={SVG_HEIGHT - 6} stroke="var(--ifm-color-emphasis-400)" strokeWidth="1" />
            <text x={toX(v)} y={SVG_HEIGHT - 0} textAnchor="middle" fontSize="8" fill="var(--ifm-color-emphasis-500)">{v}</text>
          </g>
        ))}

        {/* Labels */}
        <text x={8} y={A_Y + BAR_H / 2 + 4} fontSize="11" fill="var(--ifm-color-primary-darkest)" fontWeight="600">A</text>
        <text x={8} y={B_Y + BAR_H / 2 + 4} fontSize="11" fill="#e36209" fontWeight="600">B</text>

        {/* Interval A */}
        <rect x={toX(aStart)} y={A_Y} width={toX(aEnd) - toX(aStart)} height={BAR_H} rx="3" fill="var(--ifm-color-primary)" opacity="0.3" />
        <rect x={toX(aStart)} y={A_Y} width={toX(aEnd) - toX(aStart)} height={BAR_H} rx="3" stroke="var(--ifm-color-primary)" strokeWidth="2" fill="none" />
        {/* Drag handles */}
        <circle cx={toX(aStart)} cy={A_Y + BAR_H / 2} r="5" fill="var(--ifm-color-primary)" cursor="ew-resize" onMouseDown={handleMouseDown('aStart')} />
        <circle cx={toX(aEnd)} cy={A_Y + BAR_H / 2} r="5" fill="var(--ifm-color-primary)" cursor="ew-resize" onMouseDown={handleMouseDown('aEnd')} />
        <text x={toX(aStart)} y={A_Y - 4} textAnchor="middle" fontSize="9" fill="var(--ifm-color-primary-darkest)">{aStart}</text>
        <text x={toX(aEnd)} y={A_Y - 4} textAnchor="middle" fontSize="9" fill="var(--ifm-color-primary-darkest)">{aEnd}</text>

        {/* Interval B */}
        <rect x={toX(bStart)} y={B_Y} width={toX(bEnd) - toX(bStart)} height={BAR_H} rx="3" fill="#e36209" opacity="0.3" />
        <rect x={toX(bStart)} y={B_Y} width={toX(bEnd) - toX(bStart)} height={BAR_H} rx="3" stroke="#e36209" strokeWidth="2" fill="none" />
        <circle cx={toX(bStart)} cy={B_Y + BAR_H / 2} r="5" fill="#e36209" cursor="ew-resize" onMouseDown={handleMouseDown('bStart')} />
        <circle cx={toX(bEnd)} cy={B_Y + BAR_H / 2} r="5" fill="#e36209" cursor="ew-resize" onMouseDown={handleMouseDown('bEnd')} />
        <text x={toX(bStart)} y={B_Y - 4} textAnchor="middle" fontSize="9" fill="#e36209">{bStart}</text>
        <text x={toX(bEnd)} y={B_Y - 4} textAnchor="middle" fontSize="9" fill="#e36209">{bEnd}</text>
      </svg>
      <div className={styles.relation}>
        A <strong>{relation || '...'}</strong> B
        <span className={styles.intervals}>
          A=[{aStart}, {aEnd})  B=[{bStart}, {bEnd})
        </span>
      </div>
    </div>
  );
}

export default function AllenVisualizer(props: AllenVisualizerProps) {
  return (
    <BrowserOnly fallback={<div>Loading visualizer...</div>}>
      {() => <AllenVisualizerInner {...props} />}
    </BrowserOnly>
  );
}
