#!/usr/bin/env node
// Validate the behaviour-reference registry and its pages.
//
// Runs as part of `npm run build`, next to check-links.mjs:
//   * every claim has the required provenance fields and a unique id;
//   * every `catalog_case` names a real case in parity/catalog.json;
//   * every `<Claim id="…" />` used on a page resolves;
//   * claims older than twelve months are reported (not fatal);
//   * claims no page uses are reported (not fatal).
//
// Usage: node scripts/check-claims.mjs   (from site/)

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
// Optional overrides so the rules themselves can be exercised with fixtures.
const [claimsArg, catalogArg, pagesArg] = process.argv.slice(2);
const claimsFile = claimsArg ? resolve(claimsArg) : join(root, "reference", "claims.jsonl");
const catalogFile = catalogArg ? resolve(catalogArg) : join(root, "parity", "catalog.json");
const pagesDir = pagesArg ? resolve(pagesArg) : join(root, "site", "src", "content", "docs", "reference");

const errors = [];
const warnings = [];

const isString = (value) => typeof value === "string" && value.trim().length > 0;

/** A real, past calendar date — `2026-99-99` and `2026-02-30` both fail. */
const isRealDate = (value) => {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const parsed = new Date(`${value}T00:00:00Z`);
  if (Number.isNaN(parsed.getTime())) return false;
  if (parsed.toISOString().slice(0, 10) !== value) return false;
  return parsed.getTime() <= Date.now();
};

// --- claims -----------------------------------------------------------------
const claims = new Map();
for (const [index, line] of readFileSync(claimsFile, "utf8").split("\n").entries()) {
  const trimmed = line.trim();
  if (!trimmed) continue;
  let claim;
  try {
    claim = JSON.parse(trimmed);
  } catch (error) {
    errors.push(`reference/claims.jsonl:${index + 1}: not JSON (${error.message})`);
    continue;
  }
  const where = `reference/claims.jsonl:${index + 1} (${claim.id ?? "no id"})`;
  if (!isString(claim.id) || !/^[a-z0-9]+(-[a-z0-9]+)*$/.test(claim.id ?? "")) {
    errors.push(`${where}: id must be a kebab-case string`);
  } else if (claims.has(claim.id)) {
    errors.push(`${where}: duplicate id`);
  }
  if (!isString(claim.title)) errors.push(`${where}: title must be a non-empty string`);
  if (!["verified", "open", "superseded"].includes(claim.status)) {
    errors.push(`${where}: status must be verified, open or superseded`);
  }
  if (!isString(claim.environment?.engine) && !isString(claim.environment?.client)) {
    errors.push(`${where}: environment needs an engine or a client string`);
  }
  if (!isRealDate(claim.environment?.date)) {
    errors.push(`${where}: environment.date must be a real past calendar date (YYYY-MM-DD)`);
  }
  if (!isString(claim.method)) errors.push(`${where}: method must be a non-empty string`);
  if (!isString(claim.repro)) {
    errors.push(`${where}: repro must be a non-empty string (plan 056 requires one)`);
  }
  if (claim.status === "superseded" && !isString(claim.supersedes) && !isString(claim.superseded_by)) {
    errors.push(`${where}: a superseded claim needs supersedes or superseded_by`);
  }
  for (const field of ["notes", "repro", "catalog_case", "supersedes", "superseded_by"]) {
    if (claim[field] !== undefined && !isString(claim[field])) {
      errors.push(`${where}: ${field} must be a string when present`);
    }
  }
  claims.set(claim.id, claim);
}

// --- catalog cases ----------------------------------------------------------
const catalog = JSON.parse(readFileSync(catalogFile, "utf8"));
const caseById = new Map(catalog.cases.map((entry) => [entry.id, entry]));
for (const claim of claims.values()) {
  if (!claim.catalog_case) continue;
  const entry = caseById.get(claim.catalog_case);
  if (!entry) {
    errors.push(`claim '${claim.id}': catalog_case '${claim.catalog_case}' is not in parity/catalog.json`);
  } else if (entry.reference_claim !== claim.id) {
    errors.push(
      `claim '${claim.id}': parity case '${claim.catalog_case}' must name it in reference_claim`,
    );
  }
}

// --- pages ------------------------------------------------------------------
const used = new Set();
const pages = [];
const walk = (dir) => {
  let entries = [];
  try {
    entries = readdirSync(dir);
  } catch {
    return;
  }
  for (const entry of entries) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) walk(path);
    else if (entry.endsWith(".mdx") || entry.endsWith(".md")) pages.push(path);
  }
};
walk(pagesDir);

for (const page of pages) {
  const text = readFileSync(page, "utf8");
  const where = relative(root, page);
  for (const match of text.matchAll(/<Claim\s+id=["']([^"']+)["']/g)) {
    used.add(match[1]);
    if (!claims.has(match[1])) {
      errors.push(`${where}: <Claim id="${match[1]}" /> has no entry in reference/claims.jsonl`);
    }
  }
}
for (const claim of claims.values()) {
  if (pages.length > 0 && !used.has(claim.id)) {
    warnings.push(`claim '${claim.id}' is not used by any page yet`);
  }
}

// --- staleness --------------------------------------------------------------
for (const claim of claims.values()) {
  const then = new Date(`${claim.environment.date}T00:00:00Z`).getTime();
  const months = (Date.now() - then) / (1000 * 60 * 60 * 24 * 30.44);
  if (claim.status !== "superseded" && months > 12) {
    warnings.push(`claim '${claim.id}' was verified ${claim.environment.date} — re-verify it`);
  }
}

for (const warning of warnings) console.warn(`warning: ${warning}`);
if (errors.length > 0) {
  for (const error of errors) console.error(`error: ${error}`);
  process.exit(1);
}
console.log(`CHECK OK: ${claims.size} claims, ${used.size} referenced from ${pages.length} page(s)`);
