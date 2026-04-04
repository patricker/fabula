import React from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';

const DSL_EXAMPLE = `pattern access_after_revocation
  stage revoked
    edge ?user "access_revoked" ?resource
  stage accessed
    edge ?user "accessed" ?resource
    unless_between "revoked" "accessed"
      edge ?user "reauthorized" ?resource`;

const USE_CASES: { title: string; description: string; href: string }[] = [
  { title: 'Narrative Sifting', description: 'Surface compelling story moments in simulation output.', href: '/docs/use-cases/narrative-sifting' },
  { title: 'Observability', description: 'Detect distributed system anomalies as they propagate.', href: '/docs/use-cases/observability' },
  { title: 'Process Mining', description: 'Match expected workflows against actual event logs.', href: '/docs/use-cases/process-mining' },
  { title: 'Compliance', description: 'Flag violations the moment a rule is broken.', href: '/docs/use-cases/compliance-checking' },
  { title: 'Cybersecurity', description: 'Recognize attack sequences across temporal event streams.', href: '/docs/use-cases/cybersecurity' },
  { title: 'Simulation Monitoring', description: 'Track emergent patterns in agent-based simulations.', href: '/docs/use-cases/simulation-monitoring' },
];

const FEATURES = [
  'Zero dependencies',
  'Incremental matching',
  'Allen interval algebra',
  'Negation windows',
  'Gap analysis',
  'Surprise scoring',
  'Pattern composition',
  'DSL with TypeMapper',
  'Bring your own graph',
];

const REFERENCES = [
  { label: 'Felt', venue: 'ICIDS 2019' },
  { label: 'Winnow', venue: 'AIIDE 2021' },
  { label: 'StU', venue: 'ICIDS 2022' },
  { label: 'Allen', venue: 'CACM 1983' },
];

export default function Home(): React.JSX.Element {
  return (
    <Layout title="Fabula" description="Pattern matching over temporal graphs">
      <main style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        padding: '4rem 2rem 3rem',
        maxWidth: '900px',
        margin: '0 auto',
      }}>

        {/* Title + tagline */}
        <h1 style={{ fontSize: '2.8rem', marginBottom: '0.5rem', fontWeight: 700 }}>Fabula</h1>
        <p style={{
          fontSize: '1.3rem',
          color: 'var(--ifm-color-emphasis-800)',
          margin: '0 0 0.5rem',
          textAlign: 'center',
        }}>
          Pattern matching over temporal graphs.
        </p>
        <p style={{
          fontSize: '1.05rem',
          color: 'var(--ifm-color-emphasis-600)',
          margin: '0 0 2rem',
          textAlign: 'center',
          maxWidth: '640px',
        }}>
          Find patterns in event streams — betrayals, cascades, violations — as they unfold.
        </p>

        {/* Code on the fold */}
        <pre style={{
          width: '100%',
          padding: '1.25rem 1.5rem',
          background: 'var(--ifm-color-emphasis-100)',
          border: '1px solid var(--ifm-color-emphasis-300)',
          borderRadius: '6px',
          overflow: 'auto',
          margin: '0 0 2.5rem',
        }}>
          <code style={{
            fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace',
            fontSize: '0.9rem',
            lineHeight: 1.6,
            color: 'var(--ifm-color-emphasis-900)',
          }}>
            {DSL_EXAMPLE}
          </code>
        </pre>

        {/* Three entry points */}
        <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap', justifyContent: 'center', marginBottom: '3.5rem' }}>
          <Link className="button button--primary button--lg" to="/docs/playground/pattern-playground">
            Try the Playground
          </Link>
          <Link className="button button--secondary button--lg" to="/docs/getting-started">
            Get Started
          </Link>
          <Link className="button button--secondary button--lg" to="/docs/learn/what-is-sifting">
            Learn Sifting
          </Link>
        </div>

        {/* Use cases */}
        <section style={{ width: '100%', marginBottom: '3.5rem' }}>
          <h2 style={{ fontSize: '1.5rem', fontWeight: 600, marginBottom: '1.25rem' }}>Use cases</h2>
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(3, 1fr)',
            gap: '1rem',
            width: '100%',
          }}>
            {USE_CASES.map((uc) => (
              <Link
                key={uc.title}
                to={uc.href}
                style={{
                  display: 'block',
                  padding: '1rem 1.15rem',
                  border: '1px solid var(--ifm-color-emphasis-300)',
                  borderRadius: '6px',
                  textDecoration: 'none',
                  color: 'inherit',
                }}
              >
                <h3 style={{ fontSize: '1rem', fontWeight: 600, margin: '0 0 0.3rem' }}>{uc.title}</h3>
                <p style={{ margin: 0, fontSize: '0.9rem', color: 'var(--ifm-color-emphasis-600)' }}>{uc.description}</p>
              </Link>
            ))}
          </div>
        </section>

        {/* Features as compact list */}
        <section style={{ width: '100%', marginBottom: '3.5rem' }}>
          <h2 style={{ fontSize: '1.5rem', fontWeight: 600, marginBottom: '0.75rem' }}>Features</h2>
          <p style={{
            fontSize: '1rem',
            color: 'var(--ifm-color-emphasis-700)',
            lineHeight: 1.7,
            margin: 0,
          }}>
            {FEATURES.join('. ')}.
          </p>
        </section>

        {/* Research credibility */}
        <p style={{
          fontSize: '0.9rem',
          color: 'var(--ifm-color-emphasis-500)',
          textAlign: 'center',
          marginBottom: '2.5rem',
        }}>
          Built on {REFERENCES.map((r, i) => (
            <span key={r.label}>
              {r.label} ({r.venue}){i < REFERENCES.length - 1 ? ', ' : '.'}
            </span>
          ))}
        </p>

        {/* cargo add */}
        <pre style={{
          padding: '0.85rem 1.25rem',
          background: 'var(--ifm-color-emphasis-100)',
          border: '1px solid var(--ifm-color-emphasis-300)',
          borderRadius: '6px',
          marginBottom: '1.5rem',
          textAlign: 'center',
        }}>
          <code style={{
            fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace',
            fontSize: '0.9rem',
            color: 'var(--ifm-color-emphasis-900)',
          }}>
            cargo add fabula fabula-memory
          </code>
        </pre>

        {/* GitHub link (subtle) */}
        <Link
          href="https://github.com/patricker/fabula"
          style={{
            fontSize: '0.85rem',
            color: 'var(--ifm-color-emphasis-500)',
            textDecoration: 'none',
          }}
        >
          github.com/patricker/fabula
        </Link>

      </main>
    </Layout>
  );
}
