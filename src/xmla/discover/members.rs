#[cfg(test)]
use crate::backend::Backend;
/// MDSCHEMA_MEMBERS rowset — responds to Excel's member discovery.
///
/// Member rows are generated from actual DuckDB data (distinct dimension
/// values) plus synthetic `All` members from the semantic model.
/// No hardcoded business values remain.
use crate::backend::QueryBackend;
use crate::engine::model::{TableAccess, UserContext, effective_table_filter};
use crate::project::config::ProxyConfig;
use crate::proxy_project;
use crate::response::xml_escape;
use crate::xmla::parser::Restrictions;
use uuid::Uuid;

const MEMBER_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_NUMBER" name="LEVEL_NUMBER" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_ORDINAL" name="MEMBER_ORDINAL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_NAME" name="MEMBER_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_TYPE" name="MEMBER_TYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_GUID" name="MEMBER_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_CAPTION" name="MEMBER_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CHILDREN_CARDINALITY" name="CHILDREN_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_LEVEL" name="PARENT_LEVEL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_UNIQUE_NAME" name="PARENT_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PARENT_COUNT" name="PARENT_COUNT" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_KEY" name="MEMBER_KEY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_PLACEHOLDERMEMBER" name="IS_PLACEHOLDERMEMBER" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_DATAMEMBER" name="IS_DATAMEMBER" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="SCOPE" name="SCOPE" type="xsd:int" minOccurs="0"/>"#;

// ---- member row building ----

struct MemberRow {
    xml: String,
    #[allow(dead_code)] // read by tests only
    dimension_id: String,
    /// The `(dimension, hierarchy, level)` coordinates the request
    /// restrictions are matched against (see `coordinates_match`).
    dimension_unique_name: String,
    hierarchy_unique_name: String,
    level_unique_name: String,
    member_unique_name: String,
    parent_unique_name: Option<String>,
}

fn build_all_member_rows<B: QueryBackend + ?Sized>(
    model: &crate::engine::model::SemanticModel,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> Vec<MemberRow> {
    let project = proxy_project::project();
    let mut rows = Vec::new();
    for dim in &model.dimensions {
        let dim_table = model.dim_table_for_discovery(&dim.id);
        let access = effective_table_filter(config, user, dim_table);

        // OLS-hidden dimensions are completely excluded from member lists.
        if access == TableAccess::Hidden {
            continue;
        }

        let dim_u = dim.dimension_unique_name();
        let hier_u = dim.hierarchy_unique_name();
        let all_level_u = dim.all_level_unique_name();
        let all_member_u = dim.all_member_unique_name();

        // SSAS semantics: the (All) member's CHILDREN_CARDINALITY is the number
        // of members at the first real level (its direct children), not the
        // total leaf-row count. Keeps this rowset consistent with the axis
        // DISPLAY_INFO low word for the same member.
        let child_col = dim
            .levels
            .first()
            .map(|l| l.column.as_str())
            .unwrap_or(dim.physical_field.as_str());
        let cardinality = match &access {
            TableAccess::Filtered(sql) => backend.query_count(&format!(
                "SELECT COUNT(DISTINCT {child_col}) FROM {dim_table} WHERE {sql}"
            )),
            // Cached dictionary (plan 031): unfiltered counts are stable, so a
            // field-list refresh costs no metadata queries after the first.
            _ => model.dim_cache.get(model, dim, backend).all_cardinality,
        };
        let guid = all_member_guid(&dim.id);
        rows.push(MemberRow {
            xml: xml_member_row(
                project,
                &dim_u,
                &hier_u,
                &all_level_u,
                0,
                0,
                "All",
                &all_member_u,
                2,
                &guid,
                "All",
                cardinality,
                0,
                None,
                0,
                "All",
            ),
            dimension_id: dim.id.clone(),
            dimension_unique_name: dim_u.clone(),
            hierarchy_unique_name: hier_u.clone(),
            level_unique_name: all_level_u.clone(),
            member_unique_name: all_member_u,
            parent_unique_name: None,
        });
    }
    rows
}

fn build_leaf_member_rows<B: QueryBackend + ?Sized>(
    model: &crate::engine::model::SemanticModel,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> Vec<MemberRow> {
    let project = proxy_project::project();
    let mut rows = Vec::new();
    for dim in &model.dimensions {
        // Leveled dimensions enumerate their full hierarchy instead (see
        // build_level_member_rows); a flat unqualified leaf list under (All)
        // would contradict the level tree.
        if !dim.levels.is_empty() {
            continue;
        }
        let dim_table = model.dim_table_for_discovery(&dim.id);
        let access = effective_table_filter(config, user, dim_table);

        // OLS-hidden dimensions produce no leaf members.
        if access == TableAccess::Hidden {
            continue;
        }

        let dim_u = dim.dimension_unique_name();
        let hier_u = dim.hierarchy_unique_name();
        let leaf_level_u = dim.leaf_level_unique_name();
        let all_member_u = dim.all_member_unique_name();

        let cached_values;
        let members;
        let values: &[String] = match &access {
            TableAccess::Filtered(sql_filter) => {
                cached_values = crate::engine::dim_cache::query_leaf_values(
                    backend, dim, dim_table, sql_filter,
                );
                &cached_values
            }
            // Cached dictionary (plan 031).
            _ => {
                members = model.dim_cache.get(model, dim, backend);
                &members.leaf_values
            }
        };
        for (ordinal, val) in values.iter().enumerate() {
            let ordinal = ordinal as u32 + 1;
            let leaf_member_u = format!("{}.&[{}]", hier_u, val);
            let member_guid = leaf_member_guid(&dim.id, val);
            rows.push(MemberRow {
                xml: xml_member_row(
                    project,
                    &dim_u,
                    &hier_u,
                    &leaf_level_u,
                    1,
                    ordinal,
                    val,
                    &leaf_member_u,
                    1,
                    &member_guid,
                    val,
                    0,
                    0,
                    Some(&all_member_u),
                    1,
                    val,
                ),
                dimension_id: dim.id.clone(),
                dimension_unique_name: dim_u.clone(),
                hierarchy_unique_name: hier_u.clone(),
                level_unique_name: leaf_level_u.clone(),
                member_unique_name: leaf_member_u,
                parent_unique_name: Some(all_member_u.clone()),
            });
        }
    }
    rows
}

/// Enumerate every hierarchy level of a multi-level dimension as real
/// MDSCHEMA_MEMBERS rows (years, quarters, months, ...), each with its
/// compound-key unique name, parent link, and live child count. This is what
/// lets Excel resolve SELF probes and walk the field list for leveled dims —
/// the axis emits `[Dim].[Dim].[Year].&[2024]`-style names, so the member
/// rowset must speak the same tree.
fn build_level_member_rows<B: QueryBackend + ?Sized>(
    model: &crate::engine::model::SemanticModel,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> Vec<MemberRow> {
    let project = proxy_project::project();
    let mut rows = Vec::new();
    for dim in &model.dimensions {
        if dim.levels.is_empty() {
            continue;
        }
        let dim_table = model.dim_table_for_discovery(&dim.id);
        let access = effective_table_filter(config, user, dim_table);
        if access == TableAccess::Hidden {
            continue;
        }
        let filter_sql = match &access {
            TableAccess::Filtered(sql) => sql.as_str(),
            _ => "",
        };

        let dim_u = dim.dimension_unique_name();
        let hier_u = dim.hierarchy_unique_name();
        let all_member_u = dim.all_member_unique_name();
        // Phase 1 — distinct full paths per level (cached dictionary, plan 031;
        // RLS-filtered users query directly because cached values are
        // unfiltered). One '|' pipe-delimited string per row.
        let cached_paths;
        let members;
        let level_paths: &Vec<Vec<Vec<String>>> = if filter_sql.is_empty() {
            members = model.dim_cache.get(model, dim, backend);
            &members.level_paths
        } else {
            cached_paths =
                crate::engine::dim_cache::query_level_paths(backend, dim, dim_table, filter_sql);
            &cached_paths
        };
        // Phase 2 — emit rows. A member's child count is the number of
        // distinct next-level paths sharing its key as prefix.
        let mut ordinal = 1u32; // 0 belongs to the All member
        for (i, level) in dim.levels.iter().enumerate() {
            let tuples = &level_paths[i];
            let is_deepest = i + 1 == dim.levels.len();
            let mut child_counts: std::collections::HashMap<String, u32> =
                std::collections::HashMap::new();
            if !is_deepest {
                for t in &level_paths[i + 1] {
                    let prefix = t[..=i].join("|");
                    *child_counts.entry(prefix).or_insert(0) += 1;
                }
            }
            for t in tuples {
                let key = t.join("|");
                let name = t.last().cloned().unwrap_or_default();
                let level_u = format!("{}.[{}]", hier_u, level.name);
                let uname = format!("{level_u}.{}", key_suffix(&key));
                let (parent_u, parent_level) = if i == 0 {
                    (all_member_u.clone(), 0)
                } else {
                    let parent_key = t[..i].join("|");
                    (
                        format!(
                            "{}.[{}].{}",
                            hier_u,
                            dim.levels[i - 1].name,
                            key_suffix(&parent_key)
                        ),
                        i as u32,
                    )
                };
                let cc = if is_deepest {
                    0
                } else {
                    child_counts.get(&key).copied().unwrap_or(0)
                };
                let guid = Uuid::new_v5(&NAMESPACE, format!("level.{}.{key}", dim.id).as_bytes())
                    .to_string();
                rows.push(MemberRow {
                    xml: xml_member_row(
                        project,
                        &dim_u,
                        &hier_u,
                        &level_u,
                        i as u32 + 1,
                        ordinal,
                        &name,
                        &uname,
                        1,
                        &guid,
                        &name,
                        cc,
                        parent_level,
                        Some(&parent_u),
                        1,
                        // Compound members report the leaf key. The pipe-joined
                        // key (e9b7ab6) breaks Excel's ability to add hierarchy
                        // fields to a pivot built against the proxy (plan 048
                        // bisect; verified by comparing MDSCHEMA_MEMBERS against
                        // the last good commit).
                        &name,
                    ),
                    dimension_id: dim.id.clone(),
                    dimension_unique_name: dim_u.clone(),
                    hierarchy_unique_name: hier_u.clone(),
                    level_unique_name: level_u.clone(),
                    member_unique_name: uname,
                    parent_unique_name: Some(parent_u),
                });
                ordinal += 1;
            }
        }
    }
    rows
}

/// Stable namespace for v5 UUIDs.  Derived from "ssas-proxy" so every
/// member GUID is deterministic across runs but unique to this proxy.
const NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

fn all_member_guid(dim_id: &str) -> String {
    Uuid::new_v5(&NAMESPACE, format!("all.{dim_id}").as_bytes()).to_string()
}

fn leaf_member_guid(dim_id: &str, member_value: &str) -> String {
    Uuid::new_v5(
        &NAMESPACE,
        format!("leaf.{dim_id}.{member_value}").as_bytes(),
    )
    .to_string()
}

#[allow(clippy::too_many_arguments)] // XML row assembly mirrors the flat MDSCHEMA_MEMBERS column list
fn xml_member_row(
    project: &crate::proxy_project::ProxyProject,
    dim_u: &str,
    hier_u: &str,
    level_u: &str,
    level_num: u32,
    member_ordinal: u32,
    member_name: &str,
    member_unique_name: &str,
    member_type: u32,
    member_guid: &str,
    member_caption: &str,
    children_cardinality: u32,
    parent_level: u32,
    parent_unique_name: Option<&str>,
    parent_count: u32,
    member_key: &str,
) -> String {
    let pun = parent_unique_name
        .map(|p| {
            format!(
                "            <PARENT_UNIQUE_NAME>{}</PARENT_UNIQUE_NAME>\n",
                xml_escape(p)
            )
        })
        .unwrap_or_default();
    format!(
        r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_e}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier_e}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>{level_e}</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>{level_num}</LEVEL_NUMBER>
            <MEMBER_ORDINAL>{member_ordinal}</MEMBER_ORDINAL>
            <MEMBER_NAME>{member_name_e}</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>{mname_e}</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>{member_type}</MEMBER_TYPE>
            <MEMBER_GUID>{member_guid}</MEMBER_GUID>
            <MEMBER_CAPTION>{mcaption_e}</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>{children_cardinality}</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>{parent_level}</PARENT_LEVEL>
{pun}            <PARENT_COUNT>{parent_count}</PARENT_COUNT>
            <MEMBER_KEY>{mkey_e}</MEMBER_KEY>
            <IS_PLACEHOLDERMEMBER>false</IS_PLACEHOLDERMEMBER>
            <IS_DATAMEMBER>false</IS_DATAMEMBER>
          </row>"#,
        catalog = project.config.catalog,
        cube = project.config.cube,
        dim_e = xml_escape(dim_u),
        hier_e = xml_escape(hier_u),
        level_e = xml_escape(level_u),
        member_name_e = xml_escape(member_name),
        mname_e = xml_escape(member_unique_name),
        mcaption_e = xml_escape(member_caption),
        mkey_e = xml_escape(member_key),
    )
}

fn all_member_rows_with_backend<B: QueryBackend + ?Sized>(
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> Vec<MemberRow> {
    let project = proxy_project::project();
    build_all_member_rows(&project.model, backend, user, config)
}

fn leaf_member_rows_with_backend<B: QueryBackend + ?Sized>(
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> Vec<MemberRow> {
    let project = proxy_project::project();
    build_leaf_member_rows(&project.model, backend, user, config)
}

fn level_member_rows_with_backend<B: QueryBackend + ?Sized>(
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> Vec<MemberRow> {
    let project = proxy_project::project();
    build_level_member_rows(&project.model, backend, user, config)
}

fn key_suffix(key: &str) -> String {
    key.split('|').map(|part| format!("&[{part}]")).collect()
}

fn all_rows_with_backend<B: QueryBackend + ?Sized>(
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> Vec<MemberRow> {
    let mut rows = all_member_rows_with_backend(backend, user, config);
    // Multi-level hierarchies enumerate their full level tree (years under
    // All, quarters under years, ...); flat dims keep the plain leaf list.
    rows.extend(level_member_rows_with_backend(backend, user, config));
    rows.append(&mut leaf_member_rows_with_backend(backend, user, config));
    rows
}

// ---- filter/search helpers (reimplemented over Vec<MemberRow>) ----

fn find_member<'a>(rows: &'a [&'a MemberRow], filter: &str) -> Option<&'a MemberRow> {
    let decoded = filter.replace("&amp;", "&");
    rows.iter()
        .copied()
        .find(|r| r.member_unique_name == filter || r.member_unique_name == decoded)
}

fn find_children<'a>(rows: &'a [&'a MemberRow], parent: &str) -> Vec<&'a MemberRow> {
    let decoded = parent.replace("&amp;", "&");
    rows.iter()
        .copied()
        .filter(|r| {
            r.parent_unique_name
                .as_deref()
                .is_some_and(|pun| pun == parent || pun == decoded)
        })
        .collect()
}

/// Does a member row satisfy the Discover request's restriction list?
///
/// The mirror (SQL Server 2025 Analysis Services, measured 2026-09-23) honours
/// `DIMENSION_UNIQUE_NAME`, `HIERARCHY_UNIQUE_NAME`, and `LEVEL_UNIQUE_NAME`
/// on `MDSCHEMA_MEMBERS`: a hierarchy restriction returns that hierarchy's
/// members at every level, a level restriction returns only that level (no
/// `(All)` row), and a dimension restriction returns all of its hierarchies.
fn row_matches(restrictions: &Restrictions, row: &MemberRow) -> bool {
    super::coordinates_match(
        restrictions,
        &row.dimension_unique_name,
        Some(&row.hierarchy_unique_name),
        Some(&row.level_unique_name),
    )
}

// ---- public API ----

/// Test seam: enumerate members against the demo fixture. Production callers
/// pass the request's backend to `get_members_response_with_backend`
/// (`route_request` in `main.rs`).
#[cfg(test)]
pub fn get_members_response(member_filter: Option<&str>, tree_op: Option<i32>) -> String {
    let project = proxy_project::project();
    get_members_response_with_backend(
        member_filter,
        tree_op,
        &Restrictions::default(),
        Backend::test_fixture(),
        &UserContext::admin_default(),
        &project.config,
    )
}

pub fn get_members_response_with_backend<B: QueryBackend + ?Sized>(
    member_filter: Option<&str>,
    tree_op: Option<i32>,
    restrictions: &Restrictions,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> String {
    let all_rows = all_rows_with_backend(backend, user, config);
    // Restrictions narrow the rowset before the member/tree-op selection: the
    // reference engine intersects the two (a hierarchy restriction plus a SELF
    // probe returns the one matching row). Passing everything and filtering
    // after would be the same result, but this keeps the big row vectors out
    // of the selection path for the common one-hierarchy cache build.
    let rows: Vec<&MemberRow> = all_rows
        .iter()
        .filter(|row| row_matches(restrictions, row))
        .collect();

    let selected: Vec<&MemberRow> = match (member_filter, tree_op) {
        (Some(filter), Some(8)) => {
            // 0x08 = SELF — return only the member itself, no children
            find_member(&rows, filter).into_iter().collect()
        }
        (Some(filter), Some(1)) => {
            // 0x01 = CHILDREN — the member plus its direct children. The
            // rowset now enumerates every hierarchy level up front, so both
            // are always present in the static set.
            let mut result: Vec<&MemberRow> = Vec::new();
            if let Some(parent) = find_member(&rows, filter) {
                result.push(parent);
            }
            result.extend(find_children(&rows, filter));
            result
        }
        (Some(filter), Some(2)) => {
            // 0x02 = SIBLINGS — children of the parent of the filtered member
            if let Some(m) = find_member(&rows, filter) {
                if let Some(ref pun) = m.parent_unique_name {
                    find_children(&rows, pun)
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        (Some(filter), Some(4)) => {
            // 0x04 = PARENT — parent of the filtered member
            if let Some(m) = find_member(&rows, filter) {
                if let Some(ref pun) = m.parent_unique_name {
                    if let Some(p) = find_member(&rows, pun) {
                        vec![p]
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        (Some(filter), _) => {
            // No tree_op: return just the matching member(s)
            if let Some(m) = find_member(&rows, filter) {
                vec![m]
            } else {
                vec![]
            }
        }
        (None, _) => {
            // No filter: return every member left after the restrictions.
            rows.clone()
        }
    };

    let xml_rows: String = selected
        .iter()
        .map(|r| r.xml.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    crate::response::discover_rowset_envelope("", MEMBER_ROW_FIELDS, &xml_rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::project::with_test_project;

    fn all_rows() -> Vec<MemberRow> {
        let project = proxy_project::project();
        all_rows_with_backend(
            Backend::test_fixture(),
            &UserContext::admin_default(),
            &project.config,
        )
    }

    /// Rows under project3, whose Date dimension carries a real
    /// Year→Quarter→Month→Date hierarchy.
    fn project3_rows() -> Vec<MemberRow> {
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        with_test_project(p, || {
            let project = proxy_project::project();
            all_rows_with_backend(
                Backend::test_fixture(),
                &UserContext::admin_default(),
                &project.config,
            )
        })
    }

    fn find_row<'a>(rows: &'a [MemberRow], uname: &str) -> &'a MemberRow {
        rows.iter()
            .find(|r| r.member_unique_name == uname)
            .unwrap_or_else(|| panic!("missing row {uname}"))
    }

    fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let open = xml.find(&format!("<{tag}>"))? + tag.len() + 2;
        let close = xml[open..].find(&format!("</{tag}>"))?;
        Some(xml[open..open + close].to_string())
    }

    fn is_valid_uuid(s: &str) -> bool {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 5 {
            return false;
        }
        let lens = [8usize, 4, 4, 4, 12];
        for (i, p) in parts.iter().enumerate() {
            if p.len() != lens[i] {
                return false;
            }
            if !p.chars().all(|c| c.is_ascii_hexdigit()) {
                return false;
            }
        }
        true
    }

    #[test]
    fn generates_rows_for_both_dims() {
        let rows = all_rows();
        let dims: std::collections::HashSet<&str> =
            rows.iter().map(|r| r.dimension_id.as_str()).collect();
        assert!(dims.contains("ProductCategory"));
        assert!(dims.contains("Region"));
    }

    #[test]
    fn all_members_have_correct_type_and_no_parent() {
        let rows = all_rows();
        for r in &rows {
            if r.member_unique_name.ends_with("[All]") {
                assert!(r.xml.contains("<MEMBER_TYPE>2</MEMBER_TYPE>"));
                assert!(r.parent_unique_name.is_none());
            }
        }
    }

    #[test]
    fn leaf_members_have_parent() {
        let rows = all_rows();
        let leaf: Vec<_> = rows
            .iter()
            .filter(|r| r.member_unique_name.contains("&["))
            .collect();
        assert!(!leaf.is_empty(), "should have leaf members from DuckDB");
        for r in leaf {
            assert!(r.parent_unique_name.is_some());
        }
    }

    #[test]
    fn full_response_contains_both_dimensions() {
        let xml = get_members_response(None, None);
        assert!(xml.contains("[ProductCategory]"));
        assert!(xml.contains("[Region]"));
    }

    #[test]
    fn all_guids_are_valid_uuids() {
        let rows = all_rows();
        assert!(!rows.is_empty());
        for r in &rows {
            let Some(guid) = extract_tag(&r.xml, "MEMBER_GUID") else {
                panic!("no MEMBER_GUID in row for {}", r.member_unique_name);
            };
            assert!(
                is_valid_uuid(&guid),
                "invalid MEMBER_GUID '{guid}' in row for {}: must be 8-4-4-4-12 hex chars",
                r.member_unique_name,
            );
        }
    }

    #[test]
    fn tree_op_self_returns_only_the_member() {
        // 0x08 = SELF must return exactly the requested member — no children.
        let xml = get_members_response(Some("[Region].[Region].[All]"), Some(8));
        assert_eq!(
            xml.matches("<row>").count(),
            1,
            "SELF returns one row, got: {xml}"
        );
        assert!(
            xml.contains(">All</MEMBER_CAPTION>"),
            "SELF returns the All member itself"
        );
        assert!(
            !xml.contains("&amp;[North]"),
            "SELF must not include children"
        );

        // 0x01 = CHILDREN keeps returning member + children.
        let xml = get_members_response(Some("[Region].[Region].[All]"), Some(1));
        assert!(
            xml.contains("&amp;[North]"),
            "CHILDREN returns the child rows, got: {xml}"
        );
    }

    #[test]
    fn all_member_cardinality_counts_direct_children() {
        // SSAS semantics: the (All) member's CHILDREN_CARDINALITY is the number
        // of members at its first real level — not the total leaf-row count.
        let project = proxy_project::project();
        let rows = all_rows();
        assert!(!rows.is_empty());
        for dim in &project.model.dimensions {
            let Some(row) = rows
                .iter()
                .find(|r| r.member_unique_name == dim.all_member_unique_name())
            else {
                continue;
            };
            let cc: u32 = extract_tag(&row.xml, "CHILDREN_CARDINALITY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(0);
            let table = project.model.dim_table_for_discovery(&dim.id);
            let col = dim
                .levels
                .first()
                .map(|l| l.column.as_str())
                .unwrap_or(dim.physical_field.as_str());
            let expected = Backend::test_fixture()
                .query_count(&format!("SELECT COUNT(DISTINCT {col}) FROM {table}"));
            assert_eq!(
                cc, expected,
                "(All) of {} must report direct-children count ({col})",
                dim.id
            );
        }
    }

    #[test]
    fn leveled_dim_enumerates_every_level() {
        let rows = project3_rows();
        let years: Vec<&MemberRow> = rows
            .iter()
            .filter(|r| {
                r.member_unique_name
                    .starts_with("[Date].[Calendar].[Year].&[")
            })
            .collect();
        assert_eq!(years.len(), 11, "one row per demo year");
        let quarters: Vec<&MemberRow> = rows
            .iter()
            .filter(|r| {
                r.member_unique_name
                    .starts_with("[Date].[Calendar].[Quarter].&[")
            })
            .collect();
        let months = rows
            .iter()
            .filter(|r| {
                r.member_unique_name
                    .starts_with("[Date].[Calendar].[Month].&[")
            })
            .count();
        let days = rows
            .iter()
            .filter(|r| {
                r.member_unique_name
                    .starts_with("[Date].[Calendar].[Full Date].&[")
            })
            .count();
        assert!(!quarters.is_empty() && quarters.len().is_multiple_of(4));
        assert!(months > quarters.len());
        assert!(days > months);

        // No flat unqualified Date leaves may survive next to the level tree.
        assert!(
            !rows.iter().any(|r| r
                .member_unique_name
                .starts_with("[Date].[Full Date].[Date].[Full Date].&[[")),
            "unqualified Date leaves contradict the level tree"
        );

        // A year: level 1, parented by (All), four quarter children.
        let year = find_row(&rows, "[Date].[Calendar].[Year].&[2024]");
        assert_eq!(extract_tag(&year.xml, "LEVEL_NUMBER").as_deref(), Some("1"));
        assert_eq!(
            extract_tag(&year.xml, "PARENT_UNIQUE_NAME").as_deref(),
            Some("[Date].[Calendar].[All]")
        );
        assert_eq!(extract_tag(&year.xml, "PARENT_LEVEL").as_deref(), Some("0"));
        assert_eq!(
            extract_tag(&year.xml, "CHILDREN_CARDINALITY").as_deref(),
            Some("4")
        );

        // A compound-key quarter: level 2, parented by its year.
        let quarter = find_row(&rows, "[Date].[Calendar].[Quarter].&[2024]&[2]");
        assert_eq!(
            extract_tag(&quarter.xml, "LEVEL_NUMBER").as_deref(),
            Some("2")
        );
        assert_eq!(
            extract_tag(&quarter.xml, "PARENT_UNIQUE_NAME").as_deref(),
            Some("[Date].[Calendar].[Year].&amp;[2024]")
        );
        assert_eq!(
            extract_tag(&quarter.xml, "PARENT_LEVEL").as_deref(),
            Some("1")
        );
        assert_eq!(
            extract_tag(&quarter.xml, "CHILDREN_CARDINALITY").as_deref(),
            Some("3")
        );

        // Non-leveled dims keep their plain leaf list.
        assert!(
            rows.iter()
                .any(|r| r.member_unique_name == "[Territory].[Territory].&[Northwest]"),
            "flat dims keep unqualified leaves"
        );
    }

    #[test]
    fn tree_ops_resolve_leveled_members() {
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        with_test_project(p, || {
            let year_u = "[Date].[Calendar].[Year].&[2024]";
            let quarter_u = "[Date].[Calendar].[Quarter].&[2024]&[2]";
            let project = proxy_project::project();

            // SELF on a year: exactly one row.
            let xml = get_members_response_with_backend(
                Some(year_u),
                Some(8),
                &crate::xmla::parser::Restrictions::default(),
                Backend::test_fixture(),
                &UserContext::admin_default(),
                &project.config,
            );
            assert_eq!(xml.matches("<row>").count(), 1, "SELF year: {xml}");
            assert!(
                xml.contains("[Year].&amp;[2024]</MEMBER_UNIQUE_NAME>"),
                "self row is the level-qualified year: {xml}"
            );

            // CHILDREN on a year: self + 4 quarters.
            let xml = get_members_response_with_backend(
                Some(year_u),
                Some(1),
                &crate::xmla::parser::Restrictions::default(),
                Backend::test_fixture(),
                &UserContext::admin_default(),
                &project.config,
            );
            assert_eq!(xml.matches("<row>").count(), 5, "CHILDREN year: {xml}");

            // SELF on a compound quarter.
            let xml = get_members_response_with_backend(
                Some(quarter_u),
                Some(8),
                &crate::xmla::parser::Restrictions::default(),
                Backend::test_fixture(),
                &UserContext::admin_default(),
                &project.config,
            );
            assert_eq!(xml.matches("<row>").count(), 1, "SELF quarter: {xml}");

            // SIBLINGS of a quarter: all four under 2024.
            let xml = get_members_response_with_backend(
                Some(quarter_u),
                Some(2),
                &crate::xmla::parser::Restrictions::default(),
                Backend::test_fixture(),
                &UserContext::admin_default(),
                &project.config,
            );
            assert_eq!(xml.matches("<row>").count(), 4, "SIBLINGS quarter: {xml}");

            // PARENT of a quarter: the year row itself.
            let xml = get_members_response_with_backend(
                Some(quarter_u),
                Some(4),
                &crate::xmla::parser::Restrictions::default(),
                Backend::test_fixture(),
                &UserContext::admin_default(),
                &project.config,
            );
            assert_eq!(xml.matches("<row>").count(), 1, "PARENT quarter: {xml}");
            assert!(
                xml.contains("[Year].&amp;[2024]</MEMBER_UNIQUE_NAME>"),
                "parent is the year row: {xml}"
            );

            // Unknown key fails closed.
            let xml = get_members_response_with_backend(
                Some("[Date].[Calendar].[Year].&[1999]"),
                Some(8),
                &crate::xmla::parser::Restrictions::default(),
                Backend::test_fixture(),
                &UserContext::admin_default(),
                &project.config,
            );
            assert_eq!(xml.matches("<row>").count(), 0);
        });
    }

    /// The mirror (SQL Server 2025 Analysis Services, measured 2026-09-23)
    /// honours the dimension, hierarchy, and level restrictions on
    /// `MDSCHEMA_MEMBERS`: `[Date].[Calendar]` returns that hierarchy only,
    /// `[Date]` returns both Date hierarchies, and `[Date].[Calendar].[Year]`
    /// returns the years with no `(All)` row.
    ///
    /// Excel builds a pivot cache one hierarchy at a time, so ignoring these
    /// ships every hierarchy's members into every cache build — a 237 MB
    /// response on the 200k-member bench model where 3.7 MB is correct.
    #[test]
    fn restrictions_narrow_the_rowset() {
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        with_test_project(p, || {
            let project = proxy_project::project();
            let response = |restrictions: &Restrictions| {
                get_members_response_with_backend(
                    None,
                    None,
                    restrictions,
                    Backend::test_fixture(),
                    &UserContext::admin_default(),
                    &project.config,
                )
            };
            let rows = |xml: &str| xml.matches("<row>").count();

            let all = response(&Restrictions::default());

            let hierarchy = response(&Restrictions {
                hierarchy_unique_name: Some("[Category].[Category]".into()),
                ..Default::default()
            });
            assert!(rows(&hierarchy) > 0, "requested hierarchy has members");
            assert!(
                rows(&hierarchy) < rows(&all),
                "a restricted rowset is smaller than the full one"
            );
            assert_eq!(
                hierarchy
                    .matches("<HIERARCHY_UNIQUE_NAME>[Category].[Category]</HIERARCHY_UNIQUE_NAME>")
                    .count(),
                rows(&hierarchy),
                "every row is from the requested hierarchy"
            );
            assert!(
                !hierarchy.contains("[Date].[Calendar]"),
                "no other hierarchy leaks in: {hierarchy}"
            );

            let dimension = response(&Restrictions {
                dimension_unique_name: Some("[Date]".into()),
                ..Default::default()
            });
            assert!(rows(&dimension) > 0, "the Date dimension has members");
            assert_eq!(
                dimension
                    .matches("<DIMENSION_UNIQUE_NAME>[Date]</DIMENSION_UNIQUE_NAME>")
                    .count(),
                rows(&dimension),
                "a dimension restriction covers only that dimension"
            );
            assert!(
                !dimension.contains("<DIMENSION_UNIQUE_NAME>[Category]</DIMENSION_UNIQUE_NAME>"),
                "no other dimension leaks in: {dimension}"
            );
            // The mirror also enumerates the date role's key hierarchy
            // (`[Date].[Full Date]`) under this restriction; our rowset exposes
            // the key level inside `[Date].[Calendar]` instead (open gap, no
            // Excel gesture has asked for the key hierarchy's member list yet).
            assert!(
                dimension.contains("[Date].[Calendar]"),
                "the Calendar hierarchy is present: {dimension}"
            );

            let year_level = "[Date].[Calendar].[Year]";
            let level = response(&Restrictions {
                level_unique_name: Some(year_level.into()),
                ..Default::default()
            });
            assert_eq!(rows(&level), 11, "demo years 2020-2030: {level}");
            assert_eq!(
                level
                    .matches(&format!(
                        "<LEVEL_UNIQUE_NAME>{year_level}</LEVEL_UNIQUE_NAME>"
                    ))
                    .count(),
                rows(&level),
                "every row is at the requested level"
            );
            assert_eq!(
                level
                    .matches("<MEMBER_UNIQUE_NAME>[Date].[Calendar].[All]</MEMBER_UNIQUE_NAME>")
                    .count(),
                0,
                "a level restriction excludes the (All) member: {level}"
            );
        });
    }

    /// The mirror intersects a hierarchy (or level) restriction with the member
    /// and `TREE_OP` probe; a probe for a member outside the restricted
    /// hierarchy comes back empty rather than reaching into another hierarchy.
    #[test]
    fn restrictions_intersect_with_member_probes() {
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        with_test_project(p, || {
            let project = proxy_project::project();
            let response = |restrictions: &Restrictions, member: &'static str, tree_op: i32| {
                get_members_response_with_backend(
                    Some(member),
                    Some(tree_op),
                    restrictions,
                    Backend::test_fixture(),
                    &UserContext::admin_default(),
                    &project.config,
                )
            };

            let matching = Restrictions {
                hierarchy_unique_name: Some("[Category].[Category]".into()),
                ..Default::default()
            };
            let xml = response(&matching, "[Category].[Category].&[Books]", 8);
            assert_eq!(xml.matches("<row>").count(), 1, "SELF inside: {xml}");

            let other = Restrictions {
                hierarchy_unique_name: Some("[Territory].[Territory]".into()),
                ..Default::default()
            };
            let xml = response(&other, "[Category].[Category].&[Books]", 8);
            assert_eq!(
                xml.matches("<row>").count(),
                0,
                "a member outside the restricted hierarchy fails closed: {xml}"
            );
        });
    }
}
