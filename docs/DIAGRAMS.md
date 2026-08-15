# Architecture Diagrams

Mermaid `.mmd` files. View with any Mermaid renderer (GitHub, VS Code extension, mermaid.live, etc).

## Files

| File | Description |
|---|---|
| `current-architecture.mmd` | Pipeline as it exists now: `MDX -> QueryPlan -> SQL -> DuckDB -> XMLA` |
| `collapse-sequence.mmd` | Sequence diagram for a 2-hierarchy collapse request through the full pipeline |

## Key

- 🟢 Excel / external client
- 🔵 XMLA / SSAS compatibility (Rust)
- 🟩 Done / production
