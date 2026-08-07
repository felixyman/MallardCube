function quotedIdent(name) {
  return `"${String(name).replaceAll('"', '""')}"`;
}

function extractAllTableNames(source) {
  const re = /duckdb\.table\('([^']+)'\)/g;
  const tables = [];
  let m;
  while ((m = re.exec(source)) !== null) {
    if (!tables.includes(m[1])) tables.push(m[1]);
  }
  return tables;
}

function extractMeasureSourceColumns(source) {
  const cols = new Set();
  const re = /measure:\s+\w+\s+is\s+(\w+)\.\w+\(/g;
  let m;
  while ((m = re.exec(source)) !== null) {
    cols.add(m[1]);
  }
  return cols;
}

function extractDimensionColumns(source) {
  const cols = new Set();

  const groupByRe = /group_by:\s*([^\n}]+)/g;
  let m;
  while ((m = groupByRe.exec(source)) !== null) {
    for (const part of m[1].split(",")) {
      const col = part.trim().split(/\s+/)[0];
      if (col && col !== "aggregate") cols.add(col);
    }
  }

  const whereRe = /where:\s*([^\n}]+)/g;
  while ((m = whereRe.exec(source)) !== null) {
    const clause = m[1];
    const colRe = /\b(\w+)\s*(?==)/g;
    let cm;
    while ((cm = colRe.exec(clause)) !== null) {
      if (cm[1]) cols.add(cm[1]);
    }
  }

  return cols;
}

/**
 * Derive a minimal DuckDB in-memory schema from the Malloy source text.
 *
 * For each `duckdb.table('...')` reference found, creates a table with:
 * - columns from `measure:` declarations (DOUBLE type)
 * - columns from `group_by:` and `where:` clauses (VARCHAR type)
 *
 * This is a compile-time-only schema — it lets Malloy validate source
 * and field references without accessing the real database.
 */
function createTableSqlFromMalloySource(source) {
  const tables = extractAllTableNames(source);
  if (tables.length === 0) {
    throw new Error("Could not derive DuckDB table names from Malloy source");
  }

  const measureCols = extractMeasureSourceColumns(source);
  const dimCols = extractDimensionColumns(source);
  const allCols = new Map();

  for (const col of measureCols) allCols.set(col, "DOUBLE");
  for (const col of dimCols)    allCols.set(col, "VARCHAR");

  if (allCols.size === 0) {
    throw new Error("Could not derive any DuckDB columns from Malloy source");
  }

  const colsSql = Array.from(allCols.entries())
    .map(([name, type]) => `${quotedIdent(name)} ${type}`)
    .join(",\n       ");

  const stmts = tables.map(t =>
    `DROP TABLE IF EXISTS ${quotedIdent(t)};\nCREATE TABLE ${quotedIdent(t)} (\n       ${colsSql}\n     )`
  );

  return stmts.join(";\n\n");
}

module.exports = {
  createTableSqlFromMalloySource,
};
