# arena-lib - Project Prompt

## Priority order

1. `REPS.md` - SUPREME AUTHORITY
2. `.dev/DIRECTIVES.md`
3. This file (`.dev/PROMPT.md`)
4. `.dev/ROADMAP.md`

## What this crate is

Typed memory arena and slab allocator library. Generational indices, typed arenas (one allocation per type), interned strings, and bump allocation. Zero unsafe leakage into user code.

## Why it exists

Generational indices, typed arenas, interned strings, bump allocation. Zero unsafe leakage into user code.

## Skill areas

- allocator design
- generational indices
- string interning
- bump allocation

## Scope (1.0)

Defined in `.dev/ROADMAP.md`. The roadmap is the contract.

## Out of scope (always)

- Features requiring an async runtime hard-dependency. This crate must remain runtime-agnostic.
- Features pulling in a heavy framework dependency.
- Features that violate REPS.

## Pre-1.0 audit (mandatory before tagging 1.0)

See `.dev/ROADMAP.md` for the audit checklist. The audit must verify:

- Feature completeness vs. the roadmap
- API accuracy and stability
- Code cleanliness (no dead code, no commented-out blocks, no TODOs)
- Error hardening (every public error path documented and tested)
- Documentation completeness (every public item documented with at least one example)
- Test coverage (unit + integration + property where applicable)
- Benchmark coverage
- Cross-platform CI passing on Linux + macOS + Windows on stable and MSRV

## Versioning

Fast-track. No slow-stepping:

- 0.1.0 - scaffold
- 0.2.0 - first real implementation
- 0.5.0 - most features in place
- 0.9.0 - feature-complete, hardening
- 0.9.x - audit findings
- 1.0.0 - stable