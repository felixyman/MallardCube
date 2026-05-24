# Architecture Diagrams

Mermaid `.mmd` files. View with any Mermaid renderer (GitHub, VS Code extension, mermaid.live, etc).

## Files

| File | Description |
|---|---|
| `current-architecture.mmd` | Pipeline as it exists now: `MDX -> QueryPlan -> {Malloy, SQL} -> DuckDB -> XMLA` |
| `target-architecture.mmd` | Target state: three-layer architecture (Compatibility / Semantic / Execution) with caching and Malloy runtime |
| `migration-plan.mmd` | Phased migration: Foundation → Caching → Malloy Runtime → Production |
| `collapse-sequence.mmd` | Sequence diagram for a 2-hierarchy collapse request through the full pipeline |

## Key

- 🟢 Excel / external client
- 🔵 XMLA / SSAS compatibility (Rust)
- 🟪 Future / planned
- 🟩 Done / production
- 🟧 In progress / next
