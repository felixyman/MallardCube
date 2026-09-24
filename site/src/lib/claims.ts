// Vite resolves this at build time — the same file the Node-side check reads,
// but inlined so the bundled component needs no filesystem path.
import raw from "../../../reference/claims.jsonl?raw";

export type ClaimStatus = "verified" | "open" | "superseded";

export interface Claim {
  id: string;
  title: string;
  status: ClaimStatus;
  environment: {
    engine: string;
    compat?: string;
    client?: string;
    date: string;
  };
  method: string;
  repro?: string;
  catalog_case?: string;
  notes?: string;
}

/** `reference/claims.jsonl` at the repository root — the single source the
 * claim component and the build check both read. */
let loaded: Map<string, Claim> | undefined;

export function allClaims(): Map<string, Claim> {
  if (!loaded) {
    loaded = new Map();
    for (const line of raw.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      const claim = JSON.parse(trimmed) as Claim;
      loaded.set(claim.id, claim);
    }
  }
  return loaded;
}

export function claim(id: string): Claim {
  const found = allClaims().get(id);
  if (!found) {
    throw new Error(`unknown claim '${id}' — add it to reference/claims.jsonl`);
  }
  return found;
}

/** Months since a claim's environment date, for the staleness flag. */
export function ageInMonths(value: string): number {
  const then = new Date(`${value}T00:00:00Z`).getTime();
  if (Number.isNaN(then)) return 0;
  return (Date.now() - then) / (1000 * 60 * 60 * 24 * 30.44);
}
