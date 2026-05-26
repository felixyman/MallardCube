#!/usr/bin/env node
/// Long-lived Malloy compilation worker.
///
/// Protocol: NDJSON over stdin/stdout.
///
/// Startup: sends {"type":"ready",...} once.
/// Requests: {"id":N,"type":"compile","source":"..."}
/// Responses: {"id":N,"ok":true,"sql":"...","compile_ms":12.3}
///            {"id":N,"ok":false,"error":"...","compile_ms":12.3}

const { Runtime } = require("@malloydata/malloy");
const { DuckDBConnection } = require("@malloydata/db-duckdb");
const readline = require("readline");
const { createTableSqlFromMalloySource } = require("./proxy-schema");

async function main() {
  // Signal ready
  process.stdout.write(
    JSON.stringify({ type: "ready", version: "0.1", compiler: "malloy-node" }) + "\n"
  );

  const rl = readline.createInterface({ input: process.stdin });

  let pending = 0;

  rl.on("line", (line) => {
    if (!line.trim()) return;

    let req;
    try {
      req = JSON.parse(line);
    } catch {
      return;
    }

    if (req.type === "shutdown") {
      process.exit(0);
    }

    if (req.type !== "compile" || req.id == null) return;

    pending++;
    const start = performance.now();

    // Chain the async work — don't lose the promise
    (async () => {
      let resp;
      try {
        let conn;
        if (process.env.DUCKDB_PATH) {
          conn = new DuckDBConnection("duckdb", process.env.DUCKDB_PATH);
        } else {
          conn = new DuckDBConnection("duckdb", ":memory:");
          await conn.runSQL(createTableSqlFromMalloySource(req.source));
        }
        const runtime = new Runtime({ connection: conn });
        const q = runtime.loadQuery(req.source);
        const sql = await q.getSQL();
        resp = { id: req.id, ok: true, sql };
      } catch (err) {
        resp = {
          id: req.id,
          ok: false,
          error: String(err && err.message ? err.message : err),
        };
      }
      resp.compile_ms = +(performance.now() - start).toFixed(2);
      process.stdout.write(JSON.stringify(resp) + "\n");
      pending--;

      if (req.type === "shutdown" && pending === 0) {
        process.exit(0);
      }
    })();
  });

  // Keep process alive even if stdin closes temporarily
  process.stdin.on("end", () => {
    // Don't exit — the parent may reconnect or send more data later
  });
}

main().catch((err) => {
  process.stderr.write(String(err.message) + "\n");
  process.exit(1);
});
