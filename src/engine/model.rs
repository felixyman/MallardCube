/// Semantic model — the canonical source of truth for cube metadata.
///
/// This drives:
/// - Malloy emitter
/// - SQL emitter
/// - XMLA metadata rowsets (dimensions, hierarchies, levels, measures)
///
/// Configured from `ProxyConfig` at startup or built from hardcoded
/// defaults via `default_model()`.

use crate::engine::plan::{DimId, MeasId};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Model types
// ---------------------------------------------------------------------------

pub enum Dialect {
    DuckDB,
}

/// The shape(s) a fallback SQL query is compatible with.
/// Used to gate execution at plan time — unsupported shapes return Empty.
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackCapability {
    /// Execute for any plan shape (Total, GroupBy 1D, GroupBy 2D).
    /// The fallback SQL is expected to handle any grouping the proxy sends it.
    Universal,
    /// Only for Total (scalar, no GROUP BY) plans.
    ScalarOnly,
    /// Only for GroupBy plans with the specific declared dimension IDs.
    /// The SQL text has its own GROUP BY that must align with these dimensions.
    GroupedSpecific(Vec<DimId>),
    /// Placeholder / TODO stub — always return Empty.
    Stub,
}

impl Dialect {
    pub fn as_malloy_source_prefix(&self) -> &str {
        match self {
            Dialect::DuckDB => "duckdb.table",
        }
    }
}

// ---------------------------------------------------------------------------
// User context and role resolution
// ---------------------------------------------------------------------------

/// Access level to a table for a user.
///
/// Returned by `effective_table_filter`:
/// - `Full` — no restriction (admin bypass, or no role filter on this table).
/// - `Filtered(sql)` — a valid DuckDB WHERE clause fragment, already OR'd
///   across roles. Append with AND when emitting SQL.
/// - `Hidden` — table is completely invisible (OLS `metadata_permission: none`).
#[derive(Debug, Clone, PartialEq)]
pub enum TableAccess {
    Full,
    Filtered(String),
    Hidden,
}

/// Runtime user identity and resolved roles.
///
/// Built from a trusted header (e.g. `X-User`) by `resolve_user_context()`.
/// Threaded through plan generation and SQL emission to enforce RLS/OLS.
/// When no `auth` config is present, the proxy uses `admin_default()` which
/// bypasses all security (backward compat).
#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: String,
    pub groups: Vec<String>,
    pub roles: Vec<String>,
    pub is_administrator: bool,
}

impl UserContext {
    /// Default context when no auth is configured: full administrative access.
    /// Matching current behavior where every request sees every row.
    pub fn admin_default() -> Self {
        Self {
            user_id: String::new(),
            groups: vec![],
            roles: vec![],
            is_administrator: true,
        }
    }

    /// Deny-all context: no roles, not an administrator.
    /// Used when auth is configured but the user does not match any role.
    pub fn deny_all() -> Self {
        Self {
            user_id: String::new(),
            groups: vec![],
            roles: vec![],
            is_administrator: false,
        }
    }
}

/// Resolve a user's identity against the configured roles.
///
/// Matches `user_id` and each `group` against `role.members[].member_name`.
/// Returns a `UserContext` with the union of all matching role names and
/// whether any matched role has `model_permission: Administrator`.
///
/// When `config.auth` is `None`, returns the admin default (full access,
/// backward compat). When auth is configured but no roles match, returns
/// a deny-all context (empty roles, not admin).
pub fn resolve_user_context(
    config: &crate::project::config::ProxyConfig,
    user_id: &str,
    groups: &[String],
) -> UserContext {
    use crate::project::config::ModelPermission;

    let Some(_auth) = &config.auth else {
        return UserContext::admin_default();
    };

    let mut matched_roles: Vec<String> = Vec::new();
    let mut is_admin = false;

    for role in &config.roles {
        let member_match = role.members.iter().any(|m| {
            m.member_name == user_id || groups.iter().any(|g| g == &m.member_name)
        });
        if member_match {
            matched_roles.push(role.name.clone());
            if role.model_permission == ModelPermission::Administrator {
                is_admin = true;
            }
        }
    }

    if matched_roles.is_empty() {
        return UserContext::deny_all();
    }

    UserContext {
        user_id: user_id.to_string(),
        groups: groups.to_vec(),
        roles: matched_roles,
        is_administrator: is_admin,
    }
}

/// Compute the effective model-level permission across all matched roles.
///
/// Returns the most permissive: Administrator > Read > None.
/// If the user is an administrator, returns `Administrator` unconditionally.
pub fn effective_model_permission(
    config: &crate::project::config::ProxyConfig,
    user: &UserContext,
) -> crate::project::config::ModelPermission {
    use crate::project::config::ModelPermission;

    if user.is_administrator {
        return ModelPermission::Administrator;
    }

    let mut result = ModelPermission::None;
    let role_names: std::collections::HashSet<&str> =
        user.roles.iter().map(|s| s.as_str()).collect();

    for role in &config.roles {
        if role_names.contains(role.name.as_str()) {
            match &role.model_permission {
                ModelPermission::Administrator => return ModelPermission::Administrator,
                ModelPermission::Read => result = ModelPermission::Read,
                ModelPermission::None => {}
            }
        }
    }

    result
}

/// Compute the effective table-level access for a user on a table.
///
/// SSAS-correct union order (least restrictive wins across roles):
/// 1. Admin users always get `Full` (bypass RLS/OLS).
/// 2. If any matched role grants full access (no `table_permission` entry,
///    or `Read` with empty `filter_expression`) → `Full`.
/// 3. If any matched role grants read access with a filter → `Filtered`
///    (OR of all filters from read-granting roles). Wins over Hidden.
/// 4. If ALL matched roles hide this table (`metadata_permission: None`
///    and no role grants read/full) → `Hidden`.
/// 5. Otherwise → `Full`.
pub fn effective_table_filter(
    config: &crate::project::config::ProxyConfig,
    user: &UserContext,
    table_name: &str,
) -> TableAccess {
    use crate::project::config::ModelPermission;

    // 1. Admin bypasses all RLS/OLS.
    if user.is_administrator {
        return TableAccess::Full;
    }

    let role_names: std::collections::HashSet<&str> =
        user.roles.iter().map(|s| s.as_str()).collect();

    let mut filters: Vec<String> = Vec::new();
    let mut any_hidden = false;
    let mut any_role_grants_full = false;
    let mut any_role_grants_read = false;

    for role in &config.roles {
        if !role_names.contains(role.name.as_str()) {
            continue;
        }
        let mut role_has_permission = false;
        for tp in &role.table_permissions {
            if tp.table != table_name {
                continue;
            }
            role_has_permission = true;
            if tp.metadata_permission == ModelPermission::None {
                // OLS hide for this role.
                any_hidden = true;
            } else {
                // Read or Administrator metadata: grants read access.
                if tp.filter_expression.is_empty() {
                    // Read with no filter = full access for this role.
                    any_role_grants_full = true;
                } else {
                    any_role_grants_read = true;
                    filters.push(tp.filter_expression.clone());
                }
            }
        }
        // SSAS: no table_permission entry for this table = full access for
        // this role. Union semantics: if any role grants full, result is Full.
        if !role_has_permission {
            any_role_grants_full = true;
        }
    }

    // 2. SSAS union semantics: Full (from any role's no-entry or Read with
    //    empty filter) wins over both Hidden and Filtered.
    if any_role_grants_full {
        return TableAccess::Full;
    }

    // 3. If any role grants read access with a filter, the user sees filtered
    //    rows. This wins over Hidden because the read-granting role provides
    //    access. SSAS: least restrictive permission wins across roles.
    if any_role_grants_read {
        let combined = if filters.len() == 1 {
            filters.into_iter().next().unwrap()
        } else {
            filters
                .into_iter()
                .map(|f| format!("({})", f))
                .collect::<Vec<_>>()
                .join(" OR ")
        };
        return TableAccess::Filtered(combined);
    }

    // 4. All matched roles hide this table (no role grants read or full).
    if any_hidden {
        return TableAccess::Hidden;
    }

    // 5. No permissions at all for this table → Full.
    TableAccess::Full
}

/// A fact table that hosts measures and dimensions.
pub struct FactTable {
    pub id: String,
    pub source_name: String,
    pub table_name: String,
    pub measure_group_name: String,
}

/// A relationship between a fact table and a dimension table.
pub struct RelationshipDef {
    pub fact_table_id: String,
    pub fact_column: String,
    pub dimension_id: String,
    pub dim_table: String,
    pub dim_column: String,
}

pub struct DimensionDef {
    pub id: DimId,
    pub semantic_name: String,
    pub physical_field: String,
    /// Physical table for distinct queries (None = use primary fact table)
    pub table_name: Option<String>,
    /// True if this dimension is shared across all fact tables.
    pub shared: bool,
    /// XMLA DIMENSION_NAME / HIERARCHY_NAME
    pub caption: String,
    /// XMLA DESCRIPTION
    pub description: String,
    /// XMLA DIMENSION_IS_VISIBLE / HIERARCHY_IS_VISIBLE
    pub visible: bool,
    /// XMLA DIMENSION_ORDINAL
    pub ordinal: u32,
    /// XMLA HIERARCHY_UNIQUE_NAME relative part
    pub hierarchy_name: String,
    /// LEVEL_NAME for (All) level
    pub all_level_name: String,
    /// LEVEL_NAME for leaf level
    pub leaf_level_name: String,
    /// Cardinality hint for XMLA DIMENSION_CARDINALITY
    pub cardinality_hint: u32,
    /// True if this dimension represents a date/calendar role.
    pub is_date_role: bool,
}

impl DimensionDef {
    pub fn dimension_unique_name(&self) -> String {
        format!("[{}]", self.caption)
    }

    pub fn hierarchy_unique_name(&self) -> String {
        format!("[{}].[{}]", self.caption, self.hierarchy_name)
    }

    pub fn all_member_unique_name(&self) -> String {
        format!("[{}].[{}].[All]", self.caption, self.hierarchy_name)
    }

    pub fn all_level_unique_name(&self) -> String {
        format!("[{}].[{}].[{}]", self.caption, self.hierarchy_name, self.all_level_name)
    }

    pub fn leaf_level_unique_name(&self) -> String {
        format!("[{}].[{}].[{}]", self.caption, self.hierarchy_name, self.leaf_level_name)
    }
}

pub struct MeasureDef {
    pub id: MeasId,
    pub fact_table_idx: usize,
    pub semantic_name: String,
    pub physical_expr: String,
    pub sql_expr: String,
    /// XMLA MEASURE_NAME
    pub caption: String,
    /// XMLA MEASURE_CAPTION / MEASURE_UNQUALIFIED_CAPTION
    pub display_name: String,
    /// XMLA DESCRIPTION
    pub description: String,
    /// XMLA MEASURE_IS_VISIBLE
    pub visible: bool,
    /// XMLA MEASURE_AGGREGATOR (1=sum)
    pub aggregator: u32,
    /// XMLA MEASURE_UNITS
    pub units: String,
    /// XMLA DEFAULT_FORMAT_STRING
    pub format_string: String,
    /// XMLA MEASUREGROUP_NAME
    pub measure_group_name: String,
    /// XMLA NUMERIC_PRECISION
    pub numeric_precision: u16,
    /// XMLA NUMERIC_SCALE
    pub numeric_scale: i16,
    /// XMLA EXPRESSION
    pub expression: String,
    /// Pre-loaded fallback SQL text (from sql_fallback/*.sql).
    /// When set, execution uses this SQL directly instead of generating.
    pub sql_fallback_sql: Option<String>,
    /// Time-intelligence flag column (e.g., "ytd_flag") for date_dim join.
    pub time_flag: Option<String>,
    /// Which date-role dimension this measure binds to.
    /// When None, falls back to the global date_dim.
    pub date_dimension_id: Option<String>,
    /// Deserialized fallback capability contract.
    /// When set, takes precedence over auto-classification from SQL text.
    pub fallback_capability: Option<FallbackCapability>,
}

impl MeasureDef {
    pub fn measure_unique_name(&self) -> String {
        format!("[Measures].[{}]", self.caption)
    }
}

#[derive(Debug, Clone)]
pub struct DateDimDef {
    pub dimension_id: DimId,
    pub table_name: String,
    pub date_key_column: String,
    pub full_date_column: String,
    pub year_column: String,
    pub quarter_column: String,
    pub month_column: String,
    pub ytd_flag_column: String,
    pub prior_year_ytd_flag_column: String,
    pub current_year_flag_column: String,
    pub qtd_flag_column: String,
    pub mtd_flag_column: String,
}

pub struct SemanticModel {
    pub fact_tables: Vec<FactTable>,
    pub dialect: Dialect,
    pub dimensions: Vec<DimensionDef>,
    pub measures: Vec<MeasureDef>,
    pub relationships: Vec<RelationshipDef>,
    /// Optional date dimension for time-intelligence flag-based filtering.
    /// Default / global fallback when a measure does not specify a date role.
    pub date_dim: Option<DateDimDef>,
    /// Per-dimension date-role definitions, keyed by dimension_id.
    /// Populated from is_date_role dimensions with explicit DateDimDef metadata.
    pub date_dims: HashMap<DimId, DateDimDef>,
}

impl SemanticModel {
    pub fn fact_table(&self, idx: usize) -> &FactTable {
        &self.fact_tables[idx]
    }

    pub fn fact_table_for_measure(&self, measure_id: &str) -> &FactTable {
        let m = self.meas_def(measure_id);
        &self.fact_tables[m.fact_table_idx]
    }

    /// Backward-compatible: the primary (first) fact table's source name.
    pub fn primary_source_name(&self) -> &str {
        &self.fact_tables[0].source_name
    }

    /// Backward-compatible: the primary (first) fact table's table name.
    pub fn primary_table_name(&self) -> &str {
        &self.fact_tables[0].table_name
    }

    /// Check whether a dimension and a measure belong to compatible
    /// fact tables.  Shared dimensions are always compatible.
    /// Fact-scoped dimensions are only compatible with measures from
    /// the same fact table.
    pub fn dim_is_compatible_with_measure(&self, dim_id: &str, meas_id: &str) -> bool {
        let dim = match self.dim_def_opt(dim_id) {
            Some(d) => d,
            None => return true, // unknown dims are treated as compatible
        };
        if dim.shared {
            return true;
        }
        let meas = self.meas_def(meas_id);
        if let Some(ref dim_table) = dim.table_name {
            return self.fact_tables[meas.fact_table_idx].table_name == *dim_table;
        }
        true
    }

    /// The effective physical table for a dimension.
    /// Falls back to the primary fact table if no explicit binding.
    pub fn dim_table(&self, dim_id: &str) -> &str {
        let dim = self.dim_def(dim_id);
        dim.table_name.as_deref().unwrap_or(self.primary_table_name())
    }

    /// The physical table for member/distinct-value discovery.
    /// Uses the relationship-backed dimension table when configured,
    /// falling back to the primary fact table only when no relationship exists.
    pub fn dim_table_for_discovery(&self, dim_id: &str) -> &str {
        let dim = self.dim_def(dim_id);
        if let Some(ref table_name) = dim.table_name {
            return table_name;
        }
        if let Some(rel) = self.rel_for_dimension(dim_id) {
            return &rel.dim_table;
        }
        self.primary_table_name()
    }

    pub fn dim_def(&self, id: &str) -> &DimensionDef {
        self.dimensions.iter().find(|d| d.id == id).unwrap()
    }

    pub fn dim_def_opt(&self, id: &str) -> Option<&DimensionDef> {
        self.dimensions.iter().find(|d| d.id == id)
    }

    pub fn meas_def(&self, id: &str) -> &MeasureDef {
        self.measures.iter().find(|m| m.id == id).unwrap()
    }

    pub fn meas_def_opt(&self, id: &str) -> Option<&MeasureDef> {
        self.measures.iter().find(|m| m.id == id)
    }

    /// Find a relationship that connects this dimension to the fact table.
    pub fn rel_for_dimension(&self, dim_id: &str) -> Option<&RelationshipDef> {
        self.relationships.iter().find(|r| r.dimension_id == dim_id)
    }

    /// Resolve the DateDimDef for a measure, respecting per-measure
    /// date-role binding. Falls back to the global date_dim when the
    /// measure has no explicit date-role dimension.
    pub fn date_dim_for_measure(&self, meas_id: &str) -> Option<&DateDimDef> {
        let meas = self.meas_def_opt(meas_id)?;
        if let Some(role_id) = &meas.date_dimension_id {
            if let Some(dd) = self.date_dims.get(role_id) {
                return Some(dd);
            }
        }
        self.date_dim.as_ref()
    }
    /// Find a dimension definition by its XMLA caption.
    pub fn dim_by_caption(&self, caption: &str) -> Option<&DimensionDef> {
        self.dimensions.iter().find(|d| d.caption == caption)
    }

    /// Return the ID of the first visible measure, or None.
    pub fn default_measure_id(&self) -> Option<MeasId> {
        self.measures.iter()
            .find(|m| m.visible)
            .map(|m| m.id.clone())
    }

    /// Return the ID of the first visible measure belonging to a
    /// specific fact table (by physical table name). Falls back to
    /// the global default if no matching measure exists.
    pub fn default_measure_for_table(&self, table_name: &str) -> Option<MeasId> {
        self.measures.iter()
            .find(|m| m.visible && self.fact_tables[m.fact_table_idx].table_name == table_name)
            .or_else(|| self.measures.iter().find(|m| m.visible))
            .map(|m| m.id.clone())
    }

    /// Return the ID of the first visible dimension, or None.
    pub fn default_dimension_id(&self) -> Option<DimId> {
        self.dimensions.iter()
            .filter(|d| d.visible)
            .min_by_key(|d| d.ordinal)
            .map(|d| d.id.clone())
    }

    /// Try to find a dimension by caption, id, or an XMLA-style bracketed
    /// reference like "[Produktkategori]" or "[Region]".
    pub fn lookup_dimension(&self, text: &str) -> Option<&DimensionDef> {
        let clean = text.trim_matches(|c: char| c == '[' || c == ']');
        self.dimensions.iter().find(|d| {
            d.caption == clean
                || d.id == clean
                || d.id == text
                || d.caption == text
                || d.dimension_unique_name().contains(clean)
        })
    }

    /// Classify a measure's fallback SQL by supported shape.
    /// Returns `None` when there is no fallback SQL or sql_expr is non-null
    /// (the measure runs through normal SQL generation).
    pub fn classify_fallback(&self, meas_id: &str) -> Option<FallbackCapability> {
        let meas = self.meas_def(meas_id);
        // If the measure has an explicit capability declared, use it.
        if let Some(cap) = &meas.fallback_capability {
            return Some(cap.clone());
        }
        // Otherwise auto-classify from the SQL text.
        let sql = meas.sql_fallback_sql.as_deref()?;
        let upper = sql.to_uppercase();
        if upper.trim() == "SELECT 1 AS DUMMY;" || upper.contains("TODO") {
            return Some(FallbackCapability::Stub);
        }
        if !upper.contains("GROUP BY") {
            return Some(FallbackCapability::ScalarOnly);
        }
        // If GROUP BY appears anywhere in the SQL text, we cannot safely
        // conclude the outer result is grouped (an inner subquery may group
        // for aggregation while the outer SELECT returns one scalar).
        // Default to ScalarOnly — callers that genuinely support grouped
        // execution must carry explicit fallback_capability metadata.
        Some(FallbackCapability::ScalarOnly)
    }
}

// ---------------------------------------------------------------------------
// Default model for the current demo dataset
// ---------------------------------------------------------------------------

pub fn default_model() -> SemanticModel {
    SemanticModel {
        fact_tables: vec![
            FactTable {
                id: "default".into(),
                source_name: "faktatabell".into(),
                table_name: "faktatabell".into(),
                measure_group_name: "Faktatabell".into(),
            },
        ],
        dialect: Dialect::DuckDB,
        dimensions: vec![
            DimensionDef {
                id: "Produktkategori".into(),
                semantic_name: "produktkategori".into(),
                physical_field: "produktkategori".into(),
                table_name: None,
                shared: false,
                caption: "Produktkategori".into(),
                description: "Våra olika produkter".into(),
                visible: true,
                ordinal: 1,
                hierarchy_name: "Produktkategori".into(),
                all_level_name: "(All)".into(),
                leaf_level_name: "Produktkategori".into(),
                cardinality_hint: 50,
                is_date_role: false,
            },
            DimensionDef {
                id: "Region".into(),
                semantic_name: "region".into(),
                physical_field: "region".into(),
                table_name: None,
                shared: false,
                caption: "Region".into(),
                description: "Geografisk region".into(),
                visible: true,
                ordinal: 2,
                hierarchy_name: "Region".into(),
                all_level_name: "(All)".into(),
                leaf_level_name: "Region".into(),
                cardinality_hint: 10,
                is_date_role: false,
            },
        ],
        measures: vec![
            MeasureDef {
                id: "TotalSales".into(),
                fact_table_idx: 0,
                semantic_name: "total_forsaljning".into(),
                physical_expr: "sales.sum()".into(),
                sql_expr: "SUM(sales)".into(),
                caption: "Total Försäljning".into(),
                display_name: "Total Försäljning (SEK)".into(),
                description: "Vår totala försäljning".into(),
                visible: true,
                aggregator: 1,
                units: "SEK".into(),
                format_string: "#,##0.00 SEK".into(),
                measure_group_name: "Faktatabell".into(),
                numeric_precision: 18,
                numeric_scale: 2,
                expression: "SUM('Faktatabell'[Sales])".into(),
                sql_fallback_sql: None,
                time_flag: None,
                date_dimension_id: None,
                fallback_capability: None,
            },
        ],
        relationships: vec![],
        date_dim: None,
        date_dims: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::config::ProxyConfig;
    use crate::project::config::ModelPermission;

    fn parse_config(json: &str) -> ProxyConfig {
        serde_json::from_str(json).expect("parse config")
    }

    // -- resolve_user_context tests --

    #[test]
    fn role_resolve_no_auth_is_admin() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": []
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\user", &[]);
        assert!(ctx.is_administrator);
        assert!(ctx.roles.is_empty());
    }

    #[test]
    fn role_resolve_admin_bypass() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "Admins",
                "model_permission": "administrator",
                "members": [{"member_name": "DOMAIN\\admin", "member_type": "user"}]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\admin", &[]);
        assert!(ctx.is_administrator);
        assert_eq!(ctx.roles, vec!["Admins"]);
    }

    #[test]
    fn role_resolve_single_role_single_table() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "EU_Region",
                "model_permission": "read",
                "members": [{"member_name": "DOMAIN\\alice", "member_type": "user"}],
                "table_permissions": [{
                    "table": "sales_fact",
                    "filter_expression": "region = 'EU'"
                }]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\alice", &[]);
        assert!(!ctx.is_administrator);
        assert_eq!(ctx.roles, vec!["EU_Region"]);

        let access = effective_table_filter(&cfg, &ctx, "sales_fact");
        assert_eq!(access, TableAccess::Filtered("region = 'EU'".into()));
    }

    #[test]
    fn role_resolve_multiple_roles_union() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [
                {
                    "name": "EU_Region",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\bob", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "sales_fact",
                        "filter_expression": "region = 'EU'"
                    }]
                },
                {
                    "name": "LargeSales",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\bob", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "sales_fact",
                        "filter_expression": "amount > 1000"
                    }]
                }
            ]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\bob", &[]);
        assert_eq!(ctx.roles.len(), 2);
        assert!(ctx.roles.contains(&"EU_Region".into()));
        assert!(ctx.roles.contains(&"LargeSales".into()));

        let access = effective_table_filter(&cfg, &ctx, "sales_fact");
        match access {
            TableAccess::Filtered(sql) => {
                assert!(sql.contains("OR"), "expected OR union, got: {}", sql);
                assert!(sql.contains("region = 'EU'"));
                assert!(sql.contains("amount > 1000"));
            }
            other => panic!("expected Filtered, got {:?}", other),
        }
    }

    #[test]
    fn role_resolve_ols_hide() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "OlsRole",
                "model_permission": "read",
                "members": [{"member_name": "DOMAIN\\user", "member_type": "user"}],
                "table_permissions": [{
                    "table": "secret_table",
                    "metadata_permission": "none"
                }]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\user", &[]);
        assert!(!ctx.is_administrator);

        let access = effective_table_filter(&cfg, &ctx, "secret_table");
        assert_eq!(access, TableAccess::Hidden);
    }

    #[test]
    fn role_resolve_no_matching_role_deny_all() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "SomeRole",
                "model_permission": "read",
                "members": [{"member_name": "DOMAIN\\specific_user", "member_type": "user"}]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\unknown", &[]);
        assert!(!ctx.is_administrator);
        assert!(ctx.roles.is_empty());

        let perm = effective_model_permission(&cfg, &ctx);
        assert_eq!(perm, ModelPermission::None);
    }

    // -- effective_model_permission tests --

    #[test]
    fn role_effective_model_permission_admin() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "Admins",
                "model_permission": "administrator",
                "members": [{"member_name": "DOMAIN\\admin", "member_type": "user"}]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\admin", &[]);
        let perm = effective_model_permission(&cfg, &ctx);
        assert_eq!(perm, ModelPermission::Administrator);
    }

    #[test]
    fn role_effective_model_permission_read() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "Readers",
                "model_permission": "read",
                "members": [{"member_name": "DOMAIN\\reader", "member_type": "user"}]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\reader", &[]);
        let perm = effective_model_permission(&cfg, &ctx);
        assert_eq!(perm, ModelPermission::Read);
    }

    #[test]
    fn role_effective_model_permission_none() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "Denied",
                "model_permission": "none",
                "members": [{"member_name": "DOMAIN\\denied_user", "member_type": "user"}]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\denied_user", &[]);
        let perm = effective_model_permission(&cfg, &ctx);
        assert_eq!(perm, ModelPermission::None);
    }

    // -- effective_table_filter tests --

    #[test]
    fn role_effective_table_filter_no_filter_full() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "ReadAll",
                "model_permission": "read",
                "members": [{"member_name": "DOMAIN\\viewer", "member_type": "user"}],
                "table_permissions": [{
                    "table": "some_table",
                    "filter_expression": ""
                }]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\viewer", &[]);
        let access = effective_table_filter(&cfg, &ctx, "some_table");
        assert_eq!(access, TableAccess::Full);
    }

    #[test]
    fn role_effective_table_filter_admin_bypass() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "Admins",
                "model_permission": "administrator",
                "members": [{"member_name": "DOMAIN\\god", "member_type": "user"}],
                "table_permissions": [{
                    "table": "any_table",
                    "filter_expression": "restrictive = true",
                    "metadata_permission": "none"
                }]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\god", &[]);
        let access = effective_table_filter(&cfg, &ctx, "any_table");
        assert_eq!(access, TableAccess::Full);
    }

    #[test]
    fn role_resolve_group_membership() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "GroupRole",
                "model_permission": "read",
                "members": [{"member_name": "DOMAIN\\group1", "member_type": "group"}]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\user", &["DOMAIN\\group1".into()]);
        assert!(!ctx.is_administrator);
        assert_eq!(ctx.roles, vec!["GroupRole"]);
    }

    #[test]
    fn role_effective_table_filter_no_table_permission_full() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "PartialRole",
                "model_permission": "read",
                "members": [{"member_name": "DOMAIN\\u", "member_type": "user"}],
                "table_permissions": [{"table": "table_a", "filter_expression": "col = 1"}]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\u", &[]);
        let access = effective_table_filter(&cfg, &ctx, "other_table");
        assert_eq!(access, TableAccess::Full);
    }

    #[test]
    fn role_most_permissive_wins() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [
                {
                    "name": "Restricted",
                    "model_permission": "none",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}]
                },
                {
                    "name": "ExtraReader",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}]
                }
            ]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\multi", &[]);
        let perm = effective_model_permission(&cfg, &ctx);
        assert_eq!(perm, ModelPermission::Read);
    }

    // -- Multi-role union: Full wins over Hidden/Filtered --

    #[test]
    fn role_multi_role_filter_plus_no_entry_is_full() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [
                {
                    "name": "FilteredRole",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "sales_fact",
                        "filter_expression": "region = 'EU'"
                    }]
                },
                {
                    "name": "UnrestrictedRole",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}]
                }
            ]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\multi", &[]);
        let access = effective_table_filter(&cfg, &ctx, "sales_fact");
        assert_eq!(access, TableAccess::Full);
    }

    #[test]
    fn role_multi_role_ols_hide_plus_no_entry_is_full() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [
                {
                    "name": "OlsRole",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "secret_table",
                        "metadata_permission": "none"
                    }]
                },
                {
                    "name": "UnrestrictedRole",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}]
                }
            ]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\multi", &[]);
        let access = effective_table_filter(&cfg, &ctx, "secret_table");
        assert_eq!(access, TableAccess::Full);
    }

    #[test]
    fn role_same_role_ols_hide_plus_filter_is_hidden() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "MixedRole",
                "model_permission": "read",
                "members": [{"member_name": "DOMAIN\\u", "member_type": "user"}],
                "table_permissions": [{
                    "table": "mixed_table",
                    "filter_expression": "col = 1",
                    "metadata_permission": "none"
                }]
            }]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\u", &[]);
        let access = effective_table_filter(&cfg, &ctx, "mixed_table");
        assert_eq!(access, TableAccess::Hidden);
    }

    #[test]
    fn role_multi_role_both_ols_hide_is_hidden() {
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [
                {
                    "name": "OlsRole1",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "hidden_table",
                        "metadata_permission": "none"
                    }]
                },
                {
                    "name": "OlsRole2",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "hidden_table",
                        "metadata_permission": "none"
                    }]
                }
            ]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\multi", &[]);
        let access = effective_table_filter(&cfg, &ctx, "hidden_table");
        assert_eq!(access, TableAccess::Hidden);
    }

    #[test]
    fn role_multi_role_ols_hide_plus_rls_filter_is_filtered() {
        // SSAS union semantics: if one role hides a table (OLS) and another
        // role grants read access with a filter (RLS), the least restrictive
        // wins → Filtered (not Hidden). The user sees filtered rows.
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [
                {
                    "name": "OlsRole",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "mixed_table",
                        "metadata_permission": "none"
                    }]
                },
                {
                    "name": "RlsRole",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "mixed_table",
                        "filter_expression": "region = 'EU'",
                        "metadata_permission": "read"
                    }]
                }
            ]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\multi", &[]);
        let access = effective_table_filter(&cfg, &ctx, "mixed_table");
        assert_eq!(access, TableAccess::Filtered("region = 'EU'".to_string()));
    }

    #[test]
    fn role_multi_role_ols_hide_plus_read_no_filter_is_full() {
        // SSAS union: one role hides (OLS), another grants Read with no filter
        // (full access). Least restrictive wins → Full.
        let cfg = parse_config(r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [
                {
                    "name": "OlsRole",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "mixed_table",
                        "metadata_permission": "none"
                    }]
                },
                {
                    "name": "ReadRole",
                    "model_permission": "read",
                    "members": [{"member_name": "DOMAIN\\multi", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "mixed_table",
                        "metadata_permission": "read"
                    }]
                }
            ]
        }"#);
        let ctx = resolve_user_context(&cfg, "DOMAIN\\multi", &[]);
        let access = effective_table_filter(&cfg, &ctx, "mixed_table");
        assert_eq!(access, TableAccess::Full);
    }
}
