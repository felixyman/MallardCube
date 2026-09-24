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
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const claimsFile = join(root, "reference", "claims.jsonl");
const catalogFile = join(root, "parity", "catalog.json");
const pagesDir = join(root, "site", "src", "content", "docs", "reference");

const errors = [];
const warnings = [];

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
  if (!claim.id || !/^[a-z0-9]+(-[a-z0-9]+)*$/.test(claim.id)) {
    errors.push(`${where}: id must be kebab-case`);
  } else if (claims.has(claim.id)) {
    errors.push(`${where}: duplicate id`);
  }
  if (!claim.title) errors.push(`${where}: missing title`);
  if (!["verified", "open", "superseded"].includes(claim.status)) {
    errors.push(`${where}: status must be verified, open or superseded`);
  }
  if (!claim.environment?.engine && !claim.environment?.client) {
    errors.push(`${where}: environment needs an engine or a client`);
  }
  if (!/^\d{4}-\d{2}-\d{2}$/.test(claim.environment?.date ?? "")) {
    errors.push(`${where}: environment.date must be YYYY-MM-DD`);
  }
  if (!claim.method) errors.push(`${where}: missing method`);
  if (!claim.notes && !claim.repro) errors.push(`${where}: needs notes or repro`);
  claims.set(claim.id, claim);
}

// --- catalog cases ----------------------------------------------------------
const catalog = JSON.parse(readFileSync(catalogFile, "utf8"));
const caseIds = new Set(catalog.cases.map((entry) => entry.id));
for (const claim of claims.values()) {
  if (claim.catalog_case && !caseIds.has(claim.catalog_case)) {
    errors.push(`claim '${claim.id}': catalog_case '${claim.catalog_case}' is not in parity/catalog.json`);
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
