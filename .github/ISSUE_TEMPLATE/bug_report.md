---
name: Bug report
about: Report a crash, wrong result, or Excel/XMLA compatibility problem
title: ''
labels: bug
assignees: ''
---

<!-- Thanks for reporting! For Excel/XMLA compatibility bugs the trace is
     essential — see CONTRIBUTING.md#excel-end-to-end-checks. -->

## What happened

<!-- What you did, what you expected, what you got. Screenshots of the pivot
     are welcome. -->

## Environment

- MallardCube commit/version:
- How it runs: `cargo run` / Docker / other
- OS:
- Excel version and build (File → Account → About Excel), e.g. `16.0.20326.20144`:
- MSOLAP provider (if known):
- Project config: <!-- attach proxy-config.json, redact secrets -->

## Reproduce

1.
2.

If the bug is query-level, paste the MDX Excel sent (found in the trace):

```mdx
```

## Trace

Run with `XMLA_TRACE=1` and attach the relevant request/response pair from
`xmla-trace.jsonl` (the MDX and the returned cellset):

- [ ] Trace attached
- [ ] Raw SQL expectation checked (if the correct value is in question):

## Anything else
