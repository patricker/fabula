import React from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';

export default function Home(): React.JSX.Element {
  return (
    <Layout title="Fabula" description="Incremental pattern matching over temporal graphs">
      <main style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: 'calc(100vh - 60px)',
        padding: '2rem',
      }}>
        <h1 style={{ fontSize: '3rem', marginBottom: '0.5rem' }}>Fabula</h1>
        <p style={{ fontSize: '1.4rem', color: 'var(--ifm-color-emphasis-700)', maxWidth: '600px', textAlign: 'center' }}>
          Incremental pattern matching over temporal graphs.
          Find narrative patterns in event streams — betrayals, arcs, convergences —
          as they unfold.
        </p>
        <div style={{ display: 'flex', gap: '1rem', marginTop: '1.5rem' }}>
          <Link className="button button--primary button--lg" to="/docs/getting-started">
            Get Started
          </Link>
          <Link className="button button--secondary button--lg" href="https://github.com/your-org/fabula">
            GitHub
          </Link>
        </div>
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))',
          gap: '1.5rem',
          maxWidth: '900px',
          marginTop: '3rem',
          width: '100%',
        }}>
          <Feature title="Zero-Dep Core" description="Core library has zero external dependencies. Add only the adapters you need." />
          <Feature title="Incremental" description="Track partial matches as edges stream in. Get notified the moment a pattern completes." />
          <Feature title="Temporal" description="Allen's 13-relation interval algebra. Patterns respect time — before, during, overlaps, contains." />
          <Feature title="Negation" description="Unless-between, unless-after, unless-global. Patterns that must NOT match within a window." />
          <Feature title="Gap Analysis" description="why_not tells you exactly which clause failed and why. Debug unmatched patterns instantly." />
          <Feature title="Bring Your Own Graph" description="One trait, six methods. MemGraph, petgraph, Grafeo adapters included." />
        </div>
      </main>
    </Layout>
  );
}

function Feature({ title, description }: { title: string; description: string }) {
  return (
    <div style={{
      padding: '1.2rem',
      border: '1px solid var(--ifm-color-emphasis-300)',
      borderRadius: '8px',
    }}>
      <h3 style={{ marginBottom: '0.5rem' }}>{title}</h3>
      <p style={{ margin: 0, color: 'var(--ifm-color-emphasis-700)', fontSize: '0.95rem' }}>{description}</p>
    </div>
  );
}
