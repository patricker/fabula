/**
 * remark-code-region: Docusaurus remark plugin that replaces code block
 * content with regions extracted from compiled example files.
 *
 * Syntax:
 *   ```rust reference file=tests/getting_started.rs#build_pattern
 *   ```
 *
 * The plugin finds code blocks with "reference" in the meta string,
 * reads the file from crates/fabula-examples/, extracts the named
 * region (or the entire file if no region), and replaces the block content.
 */

import {visit} from 'unist-util-visit';
import * as fs from 'fs';
import * as path from 'path';

const EXAMPLES_ROOT = path.resolve(__dirname, '../../crates/fabula-examples');

function extractRegion(content: string, regionName: string, filePath: string): string {
  const startMarker = `// #region ${regionName}`;
  const endMarker = '// #endregion';

  const startIdx = content.indexOf(startMarker);
  if (startIdx === -1) {
    throw new Error(
      `Region "${regionName}" not found in ${filePath}\n` +
      `  Available regions: ${findRegions(content).join(', ') || '(none)'}`,
    );
  }

  const contentStart = content.indexOf('\n', startIdx) + 1;
  const endIdx = content.indexOf(endMarker, contentStart);
  if (endIdx === -1) {
    throw new Error(
      `No matching #endregion for "${regionName}" in ${filePath}`,
    );
  }

  const raw = content.slice(contentStart, endIdx);

  // Dedent: find minimum indentation across non-empty lines and strip it
  const lines = raw.split('\n');
  const nonEmpty = lines.filter((l) => l.trim().length > 0);
  if (nonEmpty.length === 0) return '';
  const minIndent = nonEmpty.reduce((min, line) => {
    const indent = line.match(/^(\s*)/)?.[1].length ?? 0;
    return Math.min(min, indent);
  }, Infinity);
  const dedented = lines.map((l) => l.slice(minIndent)).join('\n');

  return dedented.trim();
}

function findRegions(content: string): string[] {
  const regions: string[] = [];
  const re = /\/\/ #region (\S+)/g;
  let match;
  while ((match = re.exec(content)) !== null) {
    regions.push(match[1]);
  }
  return regions;
}

function readExample(filePath: string, regionName?: string): string {
  const fullPath = path.join(EXAMPLES_ROOT, filePath);
  if (!fs.existsSync(fullPath)) {
    throw new Error(
      `Example file not found: ${filePath}\n` +
      `  Resolved to: ${fullPath}\n` +
      `  EXAMPLES_ROOT: ${EXAMPLES_ROOT}`,
    );
  }
  const content = fs.readFileSync(fullPath, 'utf-8');
  return regionName
    ? extractRegion(content, regionName, filePath)
    : content.trim();
}

export default function remarkCodeRegion() {
  return (tree: any) => {
    visit(tree, 'code', (node: any) => {
      const meta: string = node.meta || '';
      if (!meta.includes('reference')) return;

      const fileMatch = meta.match(/file=(\S+)/);
      if (!fileMatch) {
        throw new Error(
          `Code block has "reference" but no file= parameter:\n` +
          `  meta: "${meta}"`,
        );
      }

      const spec = fileMatch[1];
      const hashIdx = spec.indexOf('#');
      const filePath = hashIdx >= 0 ? spec.slice(0, hashIdx) : spec;
      const region = hashIdx >= 0 ? spec.slice(hashIdx + 1) : undefined;

      node.value = readExample(filePath, region);

      // Clean meta: remove "reference" and "file=..." tokens
      node.meta =
        meta
          .replace(/\breference\b\s*/, '')
          .replace(/file=\S+\s*/, '')
          .trim() || null;
    });
  };
}
