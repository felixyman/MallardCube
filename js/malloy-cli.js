#!/usr/bin/env node
// CLI wrapper: reads Malloy source from stdin, outputs compiled SQL on stdout.
// Uses an in-memory DuckDB for schema resolution.

const { Runtime } = require("@malloydata/malloy");
const { DuckDBConnection } = require("@malloydata/db-duckdb");

async function main() {
  const conn = new DuckDBConnection("duckdb", ":memory:");
  const runtime = new Runtime({ connection: conn });

  // Create faktatabell schema matching the Rust backend
  await conn.runSQL(
    `CREATE TABLE faktatabell (
       produktkategori VARCHAR,
       region VARCHAR,
       sales DOUBLE
     )`
  );

  // Read source from stdin
  let source = "";
  process.stdin.setEncoding("utf8");
  for await (const chunk of process.stdin) {
    source += chunk;
  }

  try {
    const q = runtime.loadQuery(source);
    const sql = await q.getSQL();
    process.stdout.write(sql);
  } catch (err) {
    process.stderr.write(String(err && err.message ? err.message : err));
    process.exit(1);
  }
}

main().catch((err) => {
  process.stderr.write(String(err.message));
  process.exit(1);
});
