function quotedIdent(name) {
  return `"${String(name).replaceAll('"', '""')}"`;
}

function extractTableName(source) {
  const m = /duckdb\.table\('([^']+)'\)/.exec(source);
  if (!m) {
    throw new Error("Could not derive DuckDB table name from Malloy source");
  }
  return m[1];
}

function extractMeasureSourceColumns(source) {
  const cols = new Set();
  const re = /measure:\s+\w+\s+is\s+(\w+)\.[A-Za-z_][A-Za-z0-9_]*\(\)/g;
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
      const col = part.trim();
      if (col) {
        cols.add(col);
      }
    }
  }

  const whereRe = /where:\s*([^\n}]+)/g;
  while ((m = whereRe.exec(source)) !== null) {
    const clause = m[1];
    const colRe = /\b(\w+)\s*(?==)/g;
    let cm;
    while ((cm = colRe.exec(clause)) !== null) {
      if (cm[1]) {
        cols.add(cm[1]);
      }
    }
  }

  return cols;
}

function createTableSqlFromMalloySource(source) {
  const table = extractTableName(source);
  const columns = new Map();

  for (const dim of extractDimensionColumns(source)) {
    columns.set(dim, "VARCHAR");
  }
  for (const metric of extractMeasureSourceColumns(source)) {
    columns.set(metric, "DOUBLE");
  }

  if (columns.size === 0) {
    throw new Error("Could not derive any DuckDB columns from Malloy source");
  }

  const colsSql = Array.from(columns.entries())
    .map(([name, type]) => `${quotedIdent(name)} ${type}`)
    .join(",\n       ");

  return `DROP TABLE IF EXISTS ${quotedIdent(table)};\nCREATE TABLE ${quotedIdent(table)} (\n       ${colsSql}\n     )`;
}

module.exports = {
  createTableSqlFromMalloySource,
};
