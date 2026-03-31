import React, { useRef, useEffect, useCallback } from 'react';
import styles from './DslEditor.module.css';

export interface ParseError {
  line: number;
  column: number;
  span: [number, number];
  message: string;
}

interface DslEditorProps {
  value: string;
  onChange: (value: string) => void;
  label: string;
  error?: ParseError | null;
  readonly?: boolean;
  height?: string;
}

const KEYWORDS = /\b(pattern|stage|unless|between|after|graph|now|temporal|true|false)\b/g;
const STRINGS = /"[^"]*"/g;
const NUMBERS = /\b\d+(\.\d+)?\b/g;
const VARIABLES = /\?\w+/g;
const COMMENTS = /\/\/.*$/gm;
const OPERATORS = /(->|\.\.)/g;

function highlight(text: string): string {
  // Escape HTML first
  let html = text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');

  // Apply highlighting in order of precedence
  // Comments first (they override everything)
  html = html.replace(COMMENTS, '<span class="hl-comment">$&</span>');
  html = html.replace(STRINGS, '<span class="hl-string">$&</span>');
  html = html.replace(VARIABLES, '<span class="hl-variable">$&</span>');
  html = html.replace(KEYWORDS, '<span class="hl-keyword">$&</span>');
  html = html.replace(NUMBERS, '<span class="hl-number">$&</span>');
  html = html.replace(OPERATORS, '<span class="hl-operator">$&</span>');

  return html;
}

export default function DslEditor({
  value,
  onChange,
  label,
  error,
  readonly = false,
  height = '240px',
}: DslEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const preRef = useRef<HTMLPreElement>(null);

  const syncScroll = useCallback(() => {
    if (textareaRef.current && preRef.current) {
      preRef.current.scrollTop = textareaRef.current.scrollTop;
      preRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  useEffect(() => {
    syncScroll();
  }, [value, syncScroll]);

  return (
    <div className={styles.editorContainer}>
      <div className={styles.editorLabel}>{label}</div>
      <div className={styles.editorWrapper} style={{ height }}>
        <pre
          ref={preRef}
          className={styles.highlight}
          dangerouslySetInnerHTML={{ __html: highlight(value) + '\n' }}
        />
        <textarea
          ref={textareaRef}
          className={styles.textarea}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onScroll={syncScroll}
          readOnly={readonly}
          spellCheck={false}
        />
      </div>
      {error && (
        <div className={styles.error}>
          Line {error.line}:{error.column} — {error.message}
        </div>
      )}
    </div>
  );
}
