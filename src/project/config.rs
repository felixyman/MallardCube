/// XMLA projection config.
///
/// A small JSON file that tells the proxy how to present a semantic model to
/// Excel.  This owns the Excel/XMLA-facing presentation details (captions,
/// order, formatting, whether a dimension has an All member, etc.).
use serde::{Deserialize, Serialize};

// ---- new time-intelligence config types ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct TimeIntelligenceConfig {
    pub date_dimension: DateDimensionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub catalog: String,
    pub cube: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub table_name: String,
    #[serde(
        default = "default_dialect",
        skip_serializing_if = "is_default_dialect"
    )]
    pub dialect: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fact_tables: Vec<FactTableConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<RelationshipConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<RoleConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_intelligence: Option<TimeIntelligenceConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<DimensionConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measures: Vec<MeasureConfig>,
    /// Optional section files for large models. Each file holds the same list
    /// format as the inline array; entries are merged (inline first) and paths
    /// resolve relative to this config file. `mallard fmt` re-splits them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measures_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationships_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles_file: Option<String>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_default_dialect(dialect: &String) -> bool {
    *dialect == default_dialect()
}

fn is_default_format(format: &String) -> bool {
    *format == default_format_string()
}

fn is_default_aggregator(aggregator: &u32) -> bool {
    *aggregator == default_aggregator()
}

pub fn default_dialect() -> String {
    "duckdb".into()
}

fn default_true() -> bool {
    true
}

fn default_format_string() -> String {
    "#,##0.00".into()
}

impl ProxyConfig {
    /// Fill derived defaults so hand-written configs stay small:
    ///
    /// - captions cascade to `hierarchy_name`, `leaf_level_name`, and
    ///   `display_name`;
    /// - `all_level_name` defaults to `(All)`;
    /// - `ordinal` follows array order when omitted;
    /// - a measure's `measure_group_name` follows its fact table, and a
    ///   missing `fact_table` means the first fact table (`default`);
    /// - `physical_field` defaults to the dimension id;
    /// - `format_string` defaults to `#,##0.00`, `visible` to true.
    ///
    /// Existing configs (which specify everything) are unaffected.
    pub fn normalize(&mut self) {
        if self.dialect.is_empty() {
            self.dialect = default_dialect();
        }
        if self.source_name.is_empty() {
            self.source_name = self.table_name.clone();
        }
        let default_fact = self
            .fact_tables
            .first()
            .map(|f| f.id.clone())
            .unwrap_or_else(|| "default".into());
        let groups: std::collections::HashMap<String, String> = self
            .fact_tables
            .iter()
            .map(|f| {
                let group = if f.measure_group_name.is_empty() {
                    f.id.clone()
                } else {
                    f.measure_group_name.clone()
                };
                (f.id.clone(), group)
            })
            .collect();

        for (i, dim) in self.dimensions.iter_mut().enumerate() {
            if dim.hierarchy_name.is_empty() {
                dim.hierarchy_name = dim.caption.clone();
            }
            if dim.all_level_name.is_empty() {
                dim.all_level_name = "(All)".into();
            }
            if dim.leaf_level_name.is_empty() {
                dim.leaf_level_name = dim.caption.clone();
            }
            if dim.physical_field.is_empty() {
                dim.physical_field = dim.id.clone();
            }
            if dim.ordinal == 0 {
                dim.ordinal = i as u32 + 1;
            }
        }
        for (i, measure) in self.measures.iter_mut().enumerate() {
            if measure.display_name.is_empty() {
                measure.display_name = measure.caption.clone();
            }
            if measure.format_string.is_empty() {
                measure.format_string = default_format_string();
            }
            if measure.ordinal == 0 {
                measure.ordinal = i as u32 + 1;
            }
            let fact = measure
                .fact_table
                .clone()
                .unwrap_or_else(|| default_fact.clone());
            if measure.measure_group_name.is_empty() {
                measure.measure_group_name = groups
                    .get(&fact)
                    .cloned()
                    .unwrap_or_else(|| self.cube.clone());
            }
            measure.fact_table = Some(fact);
        }
        for fact in self.fact_tables.iter_mut() {
            if fact.source_name.is_empty() {
                fact.source_name = fact.table_name.clone();
            }
            if fact.measure_group_name.is_empty() {
                fact.measure_group_name = fact.id.clone();
            }
        }
        for rel in self.relationships.iter_mut() {
            if rel.fact_table.is_empty() {
                rel.fact_table = default_fact.clone();
            }
        }
    }
    /// Inverse of [`normalize`]: reset derived values to their omitted form so
    /// canonical output stays compact — a minimal config stays minimal.
    pub fn deminimize(&mut self) {
        if self.source_name == self.table_name {
            self.source_name.clear();
        }
        let default_fact = self
            .fact_tables
            .first()
            .map(|f| f.id.clone())
            .unwrap_or_else(|| "default".into());
        let groups: std::collections::HashMap<String, String> = self
            .fact_tables
            .iter()
            .map(|f| {
                let group = if f.measure_group_name.is_empty() {
                    f.id.clone()
                } else {
                    f.measure_group_name.clone()
                };
                (f.id.clone(), group)
            })
            .collect();
        let cube = self.cube.clone();

        for (i, dim) in self.dimensions.iter_mut().enumerate() {
            if dim.hierarchy_name == dim.caption {
                dim.hierarchy_name.clear();
            }
            if dim.leaf_level_name == dim.caption {
                dim.leaf_level_name.clear();
            }
            if dim.all_level_name == "(All)" {
                dim.all_level_name.clear();
            }
            if dim.physical_field == dim.id {
                dim.physical_field.clear();
            }
            if dim.ordinal == i as u32 + 1 {
                dim.ordinal = 0;
            }
        }
        for (i, measure) in self.measures.iter_mut().enumerate() {
            if measure.display_name == measure.caption {
                measure.display_name.clear();
            }
            if measure.ordinal == i as u32 + 1 {
                measure.ordinal = 0;
            }
            let fact = measure
                .fact_table
                .clone()
                .unwrap_or_else(|| default_fact.clone());
            let derived_group = groups.get(&fact).cloned().unwrap_or_else(|| cube.clone());
            if measure.measure_group_name == derived_group {
                measure.measure_group_name.clear();
            }
            if fact == default_fact {
                measure.fact_table = None;
            }
        }
        for fact in self.fact_tables.iter_mut() {
            if fact.source_name == fact.table_name {
                fact.source_name.clear();
            }
            if fact.measure_group_name == fact.id {
                fact.measure_group_name.clear();
            }
        }
        for rel in self.relationships.iter_mut() {
            if rel.fact_table == default_fact {
                rel.fact_table.clear();
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactTableConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_name: String,
    pub table_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub measure_group_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub trusted_proxy: bool,
    #[serde(default = "default_trusted_header")]
    pub trusted_header: String,
    /// OIDC (JWT Bearer) validation. When set, requests must carry a valid
    /// `Authorization: Bearer <token>` from this issuer; the token's claims map
    /// to a user identity + groups that resolve against the configured roles.
    #[serde(default)]
    pub oidc: Option<OidcConfig>,
}

fn default_trusted_header() -> String {
    "X-User".into()
}

/// OIDC / JWT Bearer authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    /// Token issuer (`iss` claim), e.g. `https://login.microsoftonline.com/{tenant}/v2.0`.
    pub issuer: String,
    /// Expected audience (`aud` claim), usually the client ID.
    pub audience: String,
    /// JWKS URI. When omitted, discovered from `{issuer}/.well-known/openid-configuration`.
    #[serde(default)]
    pub jwks_uri: Option<String>,
    /// Claim holding the user identity (default "sub").
    #[serde(default = "default_user_claim")]
    pub user_claim: String,
    /// Optional claim holding group names (array or string).
    #[serde(default)]
    pub group_claim: Option<String>,
    /// Optional claim holding role names (array or string).
    #[serde(default)]
    pub role_claim: Option<String>,
}

fn default_user_claim() -> String {
    "sub".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub physical_field: String,
    pub caption: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hierarchy_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub all_level_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub leaf_level_name: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ordinal: u32,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub has_all: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cardinality_hint: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_table: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub shared: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_date_role: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hierarchy_levels: Vec<HierarchyLevelConfig>,
    /// Parent-child hierarchy: a self-referencing (key, parent) pair on this
    /// dimension's table. When set, synthetic levels (`Level 01..NN`) are
    /// materialized from the recursion at project-load time and
    /// `hierarchy_levels` is ignored.
    #[serde(default)]
    pub parent_child: Option<ParentChildConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentChildConfig {
    pub key_column: String,
    pub parent_column: String,
    /// Recompute the materialized levels at every server start. Default
    /// `false`: an existing materialization is reused (no writes), so after
    /// changing dimension data set this to `true` (or drop the
    /// `{dimension_id}__pc_*` columns) to refresh.
    #[serde(default)]
    pub refresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyLevelConfig {
    /// Level name, e.g. "Year", "Quarter", "Month", "Day"
    pub name: String,
    /// SQL column that provides values for this level
    pub column: String,
    /// Distance from the root; 0 = top level (e.g. Year), 1 = next (Quarter)
    pub level_number: u32,
    /// Cardinality hint for this level
    #[serde(default)]
    pub cardinality: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasureConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sql_expr: String,
    pub caption: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub display_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(
        default = "default_format_string",
        skip_serializing_if = "is_default_format"
    )]
    pub format_string: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub units: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ordinal: u32,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_table: Option<String>,
    #[serde(
        default = "default_aggregator",
        skip_serializing_if = "is_default_aggregator"
    )]
    pub aggregator: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
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
            
            "dimensions": [{
                "id": "ProductCategory",
                
                "physical_field": "product_category",
                "caption": "ProductCategory",
                "hierarchy_name": "ProductCategory",
                "all_level_name": "(All)",
                "leaf_level_name": "ProductCategory",
                "ordinal": 1,
                "visible": true,
                "has_all": true,
                "cardinality_hint": 50
            }],
            "measures": [{
                "id": "TotalSales",
                
                
                "sql_expr": "SUM(sales)",
                "caption": "Total",
                "display_name": "Total (SEK)",
                "format_string": "0.00",
                "units": "SEK",
                "ordinal": 1,
                "visible": true,
                "measure_group_name": "FactTable"
            }]
        }"#;
        let cfg: ProxyConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(cfg.catalog, "TEST");
        assert_eq!(cfg.dimensions[0].id, "ProductCategory");
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
