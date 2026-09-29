# sqlmesh oracle

A uv-managed environment for running SQLMesh against the fixture project
`projects/upstream_marts_sqlmesh`. It exists so the contract generator can be
built and measured against a real SQLMesh rather than a guess; nothing in the
product or CI depends on it (plan 057-A, section D).

```bash
uv sync
cd ../../projects/upstream_marts_sqlmesh
uv run --project ../../oracles/sqlmesh sqlmesh plan --auto-apply
```

Pinned by `uv.lock` (SQLMesh 0.236.2, Python 3.11). The virtualenv is not
checked in, and the fixture's generated `data/`, `logs/` and `.cache/` are
ignored.
