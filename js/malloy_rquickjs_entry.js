// Entrypoint for rquickjs: exposes a single compile function.
// Bundled with esbuild into malloy-compiler.bundle.js.
//
// Uses the Malloy Runtime + MalloyConfig to compile to SQL without
// requiring a real database connection.
// The model defines an inline table source (duckdb.table), so no
// external connection is needed for compilation.

const {
  Malloy,
  Runtime,
  MalloyConfig,
  EmptyURLReader,
  DuckDBDialect,
  registerDialect,
} = require("@malloydata/malloy");

registerDialect(DuckDBDialect);

function compileMalloyToSql(source) {
  try {
    // MalloyConfig with explicit empty URL reader and no
    // connection — the model source is inline so no DB is needed.
    const config = MalloyConfig.from({
      workingDirectory: "/",
      urlReader: new EmptyURLReader(),
      isUDFCapable: false,
    });

    const runtime = new Runtime({
      config,
      urlReader: new EmptyURLReader(),
      isUDFCapable: false,
    });

    const model = runtime.loadModelSync
      ? runtime.loadModelSync(source)
      : runtime._loadModelSync(source);

    const query = model.makeQuery();
    const prepared = query.getPreparedQuery();
    const sql = prepared.getSQL();
    return { sql };
  } catch (err) {
    return { error: String(err && err.message ? err.message : err) };
  }
}

if (typeof module !== "undefined" && module.exports) {
  module.exports = { compileMalloyToSql };
}
if (typeof globalThis !== "undefined") {
  globalThis.compileMalloyToSql = compileMalloyToSql;
}
