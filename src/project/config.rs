/// Malloy-to-XMLA projection config.
///
/// A small JSON file that tells the proxy how to present a developer's
/// Malloy model to Excel.  Malloy owns the semantics; this owns the
/// Excel/XMLA-facing presentation details (captions, order, formatting,
/// whether a dimension has an All member, etc.).
use serde::Deserialize;

// ---- new time-intelligence config types ----

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct TimeIntelligenceConfig {
    pub date_dimension: DateDimensionConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DateDimensionConfig {
    /// Which dimension serves as the calendar/date dimension.
    pub dimension_id: String,
    /// The date-key column that joins to fact table date columns.
    pub date_key_column: String,
    /// The full-date column (DATE type) for flag computation.
    pub full_date_column: String,
    /// DuckDB table name for the date dimension (defaults to "date_dim").
    pub table_name: String,
    /// Flag-column names.
    #[serde(default)]
    pub flag_columns: DateFlagColumns,
}

impl Default for DateDimensionConfig {
    fn default() -> Self {
        Self {
            dimension_id: String::new(),
            date_key_column: "date_key".into(),
            full_date_column: "full_date".into(),
            table_name: "date_dim".into(),
            flag_columns: DateFlagColumns::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DateFlagColumns {
    pub year_column: String,
    pub quarter_column: String,
    pub month_column: String,
    pub ytd_flag_column: String,
    pub prior_year_ytd_flag_column: String,
    pub current_year_flag_column: String,
    pub qtd_flag_column: String,
    pub mtd_flag_column: String,
}

impl Default for DateFlagColumns {
    fn default() -> Self {
        Self {
            year_column: "year".into(),
            quarter_column: "quarter".into(),
            month_column: "month".into(),
            ytd_flag_column: "ytd_flag".into(),
            prior_year_ytd_flag_column: "prior_year_ytd_flag".into(),
            current_year_flag_column: "current_year_flag".into(),
            qtd_flag_column: "qtd_flag".into(),
            mtd_flag_column: "mtd_flag".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct MeasureTimeIntelligenceConfig {
    pub flag_column: String,
    /// Which dimension serves as the date role for this measure.
    /// When absent, the global time_intelligence.date_dimension is used.
    #[serde(default)]
    pub dimension_id: Option<String>,
}

// ---- main config types ----

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub catalog: String,
    pub cube: String,
    pub source_name: String,
    pub table_name: String,
    pub dialect: String,
    /// Deprecated since plan 027 — no longer consumed by the runtime.
    /// Kept parseable for backward compatibility with existing project configs.
    pub malloy_model_file: String,
    #[serde(default)]
    pub db_path: Option<String>,
    #[serde(default)]
    pub fact_tables: Vec<FactTableConfig>,
    #[serde(default)]
    pub relationships: Vec<RelationshipConfig>,
    #[serde(default)]
    pub roles: Vec<RoleConfig>,
    #[serde(default)]
    pub auth: Option<AuthConfig>,
    #[serde(default)]
    pub time_intelligence: Option<TimeIntelligenceConfig>,
    pub dimensions: Vec<DimensionConfig>,
    pub measures: Vec<MeasureConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactTableConfig {
    pub id: String,
    pub source_name: String,
    pub table_name: String,
    pub measure_group_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelationshipConfig {
    pub fact_table: String,
    pub fact_column: String,
    pub dimension_id: String,
    pub dim_table: String,
    pub dim_column: String,
}

/// Model-level permission for a security role.
///
/// Maps to SSAS Tabular `modelPermission`: `none`, `read`, `administrator`.
/// `Read` is the default for backward compat (existing roles without explicit
/// permission get read access). The proxy treats `readRefresh` and `refresh`
/// as equivalent to `Read` (the proxy is a read-only runtime).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ModelPermission {
    None,
    Read,
    Administrator,
}

fn default_read() -> ModelPermission {
    ModelPermission::Read
}

/// Per-table permission within a role.
///
/// `metadata_permission: None` hides the table (OLS — object-level security).
/// `filter_expression` is a DuckDB SQL fragment used at runtime for RLS.
/// `dax_filter` carries the original DAX expression from the Tabular model
/// (for documentation / future DAX-to-SQL lowering).
#[derive(Debug, Clone, Deserialize)]
pub struct TablePermissionConfig {
    pub table: String,
    #[serde(default)]
    pub filter_expression: String,
    #[serde(default)]
    pub dax_filter: Option<String>,
    #[serde(default = "default_read")]
    pub metadata_permission: ModelPermission,
}

/// A member (user or group) assigned to a role.
#[derive(Debug, Clone, Deserialize)]
pub struct RoleMemberConfig {
    pub member_name: String,
    #[serde(default)]
    pub member_type: String,
}

/// Security role with full SSAS Tabular semantics.
///
/// When `auth` is configured on the proxy, roles are enforced at runtime:
/// - `model_permission` controls overall access (`none` = deny all,
///   `read` = subject to RLS, `administrator` = bypass RLS/OLS).
/// - `table_permissions` carry DuckDB SQL filter predicates for RLS and
///   `metadata_permission` for OLS (table hiding).
/// - Multiple roles are unioned (OR semantics across roles, most permissive
///   `model_permission` wins).
///
/// When no `auth` is configured, roles are informational only (backward
/// compat). The proxy emits a startup warning if roles are present without
/// auth.
#[derive(Debug, Clone, Deserialize)]
pub struct RoleConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_read")]
    pub model_permission: ModelPermission,
    #[serde(default)]
    pub members: Vec<RoleMemberConfig>,
    #[serde(default)]
    pub table_permissions: Vec<TablePermissionConfig>,
}

/// Authentication configuration for the trusted-proxy boundary.
///
/// When `trusted_proxy` is `true`, the proxy reads the authenticated user
/// identity from `trusted_header` (default `X-User`) and resolves roles
/// against that identity. Place a reverse proxy (IIS/nginx) in front that
/// terminates actual authentication (Windows Auth / Kerberos / Basic) and
/// sets the trusted header.
///
/// When `auth` is `None` (or absent) in `ProxyConfig`, the proxy operates
/// in admin-default mode: no user context is built, all requests see all
/// data, and roles are informational-only.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub trusted_proxy: bool,
    #[serde(default = "default_trusted_header")]
    pub trusted_header: String,
}

fn default_trusted_header() -> String {
    "X-User".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct DimensionConfig {
    pub id: String,
    /// Deprecated since plan 027 — no longer consumed by the runtime.
    /// Kept parseable for backward compatibility with existing project configs.
    pub malloy_name: String,
    pub physical_field: String,
    pub caption: String,
    #[serde(default)]
    pub description: String,
    pub hierarchy_name: String,
    pub all_level_name: String,
    pub leaf_level_name: String,
    pub ordinal: u32,
    pub visible: bool,
    pub has_all: bool,
    pub cardinality_hint: u32,
    #[serde(default)]
    pub fact_table: Option<String>,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub is_date_role: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeasureConfig {
    pub id: String,
    /// Deprecated since plan 027 — no longer consumed by the runtime.
    /// Kept parseable for backward compatibility with existing project configs.
    pub malloy_name: String,
    pub physical_expr: String,
    pub sql_expr: String,
    pub caption: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    pub format_string: String,
    pub units: String,
    pub ordinal: u32,
    pub visible: bool,
    #[serde(default)]
    pub fact_table: Option<String>,
    #[serde(default = "default_aggregator")]
    pub aggregator: u32,
    pub measure_group_name: String,
    #[serde(default = "default_precision")]
    pub numeric_precision: u16,
    #[serde(default = "default_scale")]
    pub numeric_scale: i16,
    #[serde(default)]
    pub expression: String,
    #[serde(default)]
    pub sql_fallback_file: Option<String>,
    #[serde(default)]
    pub time_intelligence: Option<MeasureTimeIntelligenceConfig>,
    #[serde(default)]
    pub fallback_capability: Option<String>,
}

fn default_aggregator() -> u32 {
    1
}
fn default_precision() -> u16 {
    18
}
fn default_scale() -> i16 {
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_config() {
        let json = r#"{
            "catalog": "TEST",
            "cube": "TestCube",
            "source_name": "test",
            "table_name": "test_table",
            "dialect": "duckdb",
            "malloy_model_file": "model.malloy",
            "dimensions": [{
                "id": "Produktkategori",
                "malloy_name": "produktkategori",
                "physical_field": "produktkategori",
                "caption": "Produktkategori",
                "hierarchy_name": "Produktkategori",
                "all_level_name": "(All)",
                "leaf_level_name": "Produktkategori",
                "ordinal": 1,
                "visible": true,
                "has_all": true,
                "cardinality_hint": 50
            }],
            "measures": [{
                "id": "TotalSales",
                "malloy_name": "total_forsaljning",
                "physical_expr": "sales.sum()",
                "sql_expr": "SUM(sales)",
                "caption": "Total",
                "display_name": "Total (SEK)",
                "format_string": "0.00",
                "units": "SEK",
                "ordinal": 1,
                "visible": true,
                "measure_group_name": "Faktatabell"
            }]
        }"#;
        let cfg: ProxyConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(cfg.catalog, "TEST");
        assert_eq!(cfg.dimensions[0].id, "Produktkategori");
        assert_eq!(cfg.measures[0].caption, "Total");
    }

    #[test]
    fn time_intelligence_config_deserializes_with_defaults() {
        let json = r#"{
            "catalog": "TEST",
            "cube": "TestCube",
            "source_name": "test",
            "table_name": "test_table",
            "dialect": "duckdb",
            "malloy_model_file": "model.malloy",
            "dimensions": [],
            "measures": [],
            "time_intelligence": {
                "date_dimension": {
                    "dimension_id": "Date",
                    "date_key_column": "date_key",
                    "full_date_column": "full_date"
                }
            }
        }"#;
        let cfg: ProxyConfig = serde_json::from_str(json).expect("parse");
        let ti = cfg.time_intelligence.expect("time_intelligence present");
        let dd = &ti.date_dimension;
        assert_eq!(dd.dimension_id, "Date");
        assert_eq!(dd.date_key_column, "date_key");
        assert_eq!(dd.full_date_column, "full_date");
        assert_eq!(dd.table_name, "date_dim"); // default
        assert_eq!(dd.flag_columns.year_column, "year"); // default
        assert_eq!(dd.flag_columns.ytd_flag_column, "ytd_flag"); // default
    }

    #[test]
    fn time_intelligence_defaults_backward_compat() {
        let json = r#"{
            "catalog": "TEST",
            "cube": "TestCube",
            "source_name": "test",
            "table_name": "test_table",
            "dialect": "duckdb",
            "malloy_model_file": "model.malloy",
            "dimensions": [],
            "measures": []
        }"#;
        let cfg: ProxyConfig = serde_json::from_str(json).expect("parse");
        assert!(
            cfg.time_intelligence.is_none(),
            "omitting time_intelligence should default to None"
        );
    }

    #[test]
    fn role_config_backward_compat() {
        let json = r#"{
            "catalog": "TEST",
            "cube": "TestCube",
            "source_name": "test",
            "table_name": "test_table",
            "dialect": "duckdb",
            "malloy_model_file": "model.malloy",
            "dimensions": [],
            "measures": [],
            "roles": [{"name": "ReaderRole", "description": "Read only"}]
        }"#;
        let cfg: ProxyConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(cfg.roles.len(), 1);
        assert_eq!(cfg.roles[0].name, "ReaderRole");
        assert_eq!(cfg.roles[0].description, "Read only");
        // Defaults: model_permission = Read, empty members, empty table_permissions
        assert_eq!(cfg.roles[0].model_permission, ModelPermission::Read);
        assert!(cfg.roles[0].members.is_empty());
        assert!(cfg.roles[0].table_permissions.is_empty());
    }

    #[test]
    fn role_config_full_parse() {
        let json = r#"{
            "catalog": "TEST",
            "cube": "TestCube",
            "source_name": "test",
            "table_name": "test_table",
            "dialect": "duckdb",
            "malloy_model_file": "model.malloy",
            "dimensions": [],
            "measures": [],
            "roles": [{
                "name": "AdminRole",
                "description": "Full access admin",
                "model_permission": "administrator",
                "members": [
                    {"member_name": "DOMAIN\\admin", "member_type": "user"},
                    {"member_name": "DOMAIN\\admins", "member_type": "group"}
                ],
                "table_permissions": [{
                    "table": "sales_fact",
                    "filter_expression": "region = 'EU'",
                    "metadata_permission": "read"
                }]
            }]
        }"#;
        let cfg: ProxyConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(cfg.roles.len(), 1);
        let role = &cfg.roles[0];
        assert_eq!(role.name, "AdminRole");
        assert_eq!(role.model_permission, ModelPermission::Administrator);
        assert_eq!(role.members.len(), 2);
        assert_eq!(role.members[0].member_name, "DOMAIN\\admin");
        assert_eq!(role.members[0].member_type, "user");
        assert_eq!(role.members[1].member_name, "DOMAIN\\admins");
        assert_eq!(role.members[1].member_type, "group");
        assert_eq!(role.table_permissions.len(), 1);
        let tp = &role.table_permissions[0];
        assert_eq!(tp.table, "sales_fact");
        assert_eq!(tp.filter_expression, "region = 'EU'");
        assert_eq!(tp.metadata_permission, ModelPermission::Read);
        assert!(tp.dax_filter.is_none());
    }

    #[test]
    fn auth_config_parse_defaults() {
        let json = r#"{
            "catalog": "TEST",
            "cube": "TestCube",
            "source_name": "test",
            "table_name": "test_table",
            "dialect": "duckdb",
            "malloy_model_file": "model.malloy",
            "dimensions": [],
            "measures": [],
            "auth": {
                "trusted_proxy": true
            }
        }"#;
        let cfg: ProxyConfig = serde_json::from_str(json).expect("parse");
        let auth = cfg.auth.expect("auth present");
        assert!(auth.trusted_proxy);
        assert_eq!(auth.trusted_header, "X-User");
    }
}
