# scripts/vm — the VM-side capture harness

These run on the Windows VM (session 1). The repo is the source of truth; the
VM copies live in `C:\Users\Public\Documents\parity\`. The two sets are
content-identical (the repo's files use LF, the VM's CRLF — both fine).

## First, always

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File vm-ready.ps1
```

`VM READY` means SSAS, IIS, the 8090 mirror listener, the WinINET posture and
ADOMD are all in order. It is idempotent — re-run it freely.

## The Excel-level gates

| script | what it does |
|---|---|
| `sweep3.ps1` | 11 Excel COM pivot specs against `-Source proxy` or `-Source mirror`, writes a text report |
| `sweep2.ps1` | the earlier sweep; same contract |
| `sweep-diff.ps1` | runs a sweep and diffs against the versioned baseline, filtering the one known flaky line |

Baselines are versioned as `parity/sweep*-baseline.txt` and copied beside the
scripts on the VM. The only line expected to move between runs is
`two_hier_same_dim`'s COM error message, which alternates between
`Exception from HRESULT: 0x800A03EC` and `Unable to set the Orientation
property...`; that spec fails against both engines either way, so the diff
runner reports it as known and fails on anything else.

## UI Automation

- `uia-probe.ps1` — dump a window's accessibility tree (names, automation ids,
  classes, rects, enabled/offscreen) or list windows. This replaces
  screenshots when something blocks a run.
- `uia-click.ps1` — click an element by name or automation id, foregrounding
  the window first (Office dialogs ignore clicks when inactive). `-DryRun`
  prints the target and clicks nothing; always dry-run a new sequence.

The date-filter dialog sequence — the one UI path that COM cannot drive — is
the next thing to build on these.

## Moving files between the VM and the host

The host's firewall opens only 8080 to the VM (the proxy), and the mirror's
8090 listener only speaks XMLA, so use the VM's Python for a one-off file
server when files need to move:

```powershell
Start-Process powershell -ArgumentList '-NoProfile','-Command',
  'python -m http.server 8099 --directory C:\Users\Public\Documents\parity' -WindowStyle Hidden
```

Then on the host:

```sh
curl -s http://192.168.124.172:8099/sweep3.ps1 -o scripts/vm/sweep3.ps1
```

which moves the exact bytes with no copy-paste. Stop it when done: it serves
the parity directory to the whole virtual network.

## The mirror chain

Excel refuses the pump over plain HTTP, so `pump-proxy2.ps1` listens on
`127.0.0.1:8090` and forwards to `https://localhost:8443`. It does not survive
a reboot; `vm-ready.ps1` starts it. `relay.py` is the logging variant
(8095 → 8090, request/response pairs under `C:\Users\Public\Documents\relay\`)
used to capture exactly what Excel sends to the reference.
