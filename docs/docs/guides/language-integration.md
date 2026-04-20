---
sidebar_position: 11
title: Language Integration
description: Using fabula from JavaScript, Python, C, and other languages
---

# Language Integration

Fabula's core is a Rust library. This page explains your options for integrating it from other languages -- the tradeoffs between WASM, FFI, service wrappers, and embedded DSL -- and walks through a concrete WASM quickstart so you can see at least one path end-to-end.

## Quick Summary

| Approach | Languages | Maturity | Use When |
|----------|-----------|----------|----------|
| **WASM** | JavaScript, TypeScript, any WASM host | Shipping (powers this site's playgrounds) | Browser apps, Node.js services, Deno |
| **C FFI** | C, C++, Go (via cgo), Python (via ctypes) | Planned | Native integration, game engines |
| **PyO3** | Python | Planned | Data science, Jupyter notebooks, scripting |
| **Embedded DSL** | Any language with a text parser | Available now | Define patterns as `.fabula` text files, evaluate via WASM or FFI |
| **Service wrapper** | Any language via HTTP/gRPC | Roll your own | Microservice architecture, language-agnostic |

## WASM quickstart (5 minutes)

The fastest way to use fabula from JavaScript:

1. **Install the WASM target** (once):
   ```bash
   rustup target add wasm32-unknown-unknown
   cargo install wasm-pack
   ```
2. **Build the fabula-wasm crate** from the repo root:
   ```bash
   wasm-pack build --target web crates/fabula-wasm
   ```
   This produces `crates/fabula-wasm/pkg/` with `.wasm`, `.js`, and `.d.ts` files.
3. **Import and call from your JS app:**
   ```javascript
   import init, { evaluate_batch } from './pkg/fabula_wasm.js';
   await init();
   const result = JSON.parse(evaluate_batch(patternDsl, graphDsl));
   console.log(result.matches);
   ```
4. **Verify against a known pattern.** Use one from the [interactive playground](/docs/playground/pattern-playground) -- if you get the same matches in your JS app, the WASM build is wired correctly.

That's enough to prototype. For Node.js, swap `--target web` for `--target nodejs` and import from `fabula_wasm.js` the same way. Performance, limitations, and production considerations are discussed below.

## WASM (JavaScript / TypeScript)

The `fabula-wasm` crate compiles to WebAssembly via `wasm-bindgen`. This is how the interactive playgrounds on this documentation site work.

### Building

```bash
# Install the wasm32 target if you haven't
rustup target add wasm32-unknown-unknown

# Build the WASM package
wasm-pack build --target web crates/fabula-wasm
```

This produces a `pkg/` directory with `.wasm`, `.js`, and `.d.ts` files ready for import.

### API Surface

The core WASM evaluation functions accept DSL text and return JSON:

- `evaluate_batch(pattern_dsl, graph_dsl)` -- batch evaluation, returns matches
- `evaluate_incremental(pattern_dsl, graph_dsl)` -- step-by-step replay
- `why_not(pattern_dsl, graph_dsl)` -- gap analysis

Additional utility functions (`parse_and_validate_pattern`, `parse_and_validate_graph`, `allen_relation`) are also available for validation and temporal reasoning.

```javascript
import init, { evaluate_batch } from './pkg/fabula_wasm.js';

await init();

const result = evaluate_batch(
  `pattern lateral_movement {
    stage e1 { e1.type = "login"  e1.host -> ?host_a  e1.cred -> ?cred }
    stage e2 { e2.type = "login"  e2.host -> ?host_b  e2.cred -> ?cred }
    temporal e1 before e2
  }`,
  `graph {
    @1 ev1.type = "login"  @1 ev1.host = "web01"  @1 ev1.cred = "admin"
    @5 ev2.type = "login"  @5 ev2.host = "db01"   @5 ev2.cred = "admin"
  }`
);

console.log(JSON.parse(result));
// { ok: true, matches: [{ pattern: "lateral_movement", bindings: { "?cred": "Str(\"admin\")", ... } }] }
// Note: binding values use Rust Debug format (e.g., Str("admin"), Node("web01"))
```

### Node.js / Deno

Use `--target nodejs` for Node.js:

```bash
wasm-pack build --target nodejs crates/fabula-wasm
```

### Limitations

- WASM bindings use the in-memory graph adapter (`MemGraph`). For custom data sources, you need Rust.
- Pattern registration is per-call (no persistent engine state across WASM calls).
- Performance is ~2-5x slower than native Rust due to WASM overhead.

## Embedded DSL (Any Language)

If your language can call WASM or a C library, you can define patterns as `.fabula` text files and evaluate them without writing Rust:

1. Write patterns in `.fabula` files using the [DSL syntax](/docs/reference/dsl)
2. Define your graph data as edge tuples
3. Call `evaluate_batch()` or `evaluate_incremental()` via WASM or FFI

This is the **recommended approach for non-Rust teams**. The DSL is the same syntax used in the interactive playgrounds.

## Python (Planned)

Python bindings via [PyO3](https://pyo3.rs/) are planned but not yet available. In the meantime:

- Use WASM via [wasmtime-py](https://github.com/bytecodealliance/wasmtime-py) or [wasmer-python](https://github.com/wasmerio/wasmer-python)
- Or wrap the WASM build in a thin HTTP service and call from Python

## C / C++ / Go (Planned)

C FFI bindings via `cbindgen` are planned. This would enable:
- Unity/Unreal integration via C plugins
- Go integration via `cgo`
- Any language with C interop

## Game Engine Integration

### Current state

No official game engine plugins exist yet. The recommended path for game developers:

1. **Godot (GDScript/C#):** Compile to WASM, call via GDExtension (Godot 4)
2. **Unity (C#):** Compile to WASM, call via JavaScript interop in WebGL builds; or await C FFI for native builds
3. **Unreal (C++):** Await C FFI, or embed Rust via `cxx` crate
4. **Bevy / custom Rust engine:** Use fabula directly as a Rust dependency (zero friction)

### Pattern-as-config workflow

For game teams that don't write Rust, the recommended workflow is:

1. Author patterns in `.fabula` text files (the DSL)
2. Load and evaluate at runtime via WASM or FFI
3. React to match results in your game logic layer

This separates pattern authoring (designers, in DSL) from pattern evaluation (engine, in Rust/WASM).

## Choosing an Approach

**"I want the fastest path to trying fabula"** -- Use the [interactive playgrounds](/docs/playground/pattern-playground). No installation needed.

**"I'm building a web app"** -- WASM. Build `fabula-wasm`, import in your frontend or Node.js backend.

**"I'm a Python data scientist"** -- WASM via wasmtime-py, or wrap in an HTTP service.

**"I'm building a game in Unity/Godot"** -- WASM for prototyping; await C FFI for production.

**"I'm a Rust developer"** -- Use fabula directly. See [Getting Started](/docs/getting-started).
