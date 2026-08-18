/// MDSCHEMA_MEMBERS rowset — responds to Excel's member discovery.
///
/// Member rows are generated from actual DuckDB data (distinct dimension
/// values) plus synthetic `All` members from the semantic model.
/// No hardcoded business values remain.
use crate::backend::{Backend, QueryBackend};
use crate::engine::model::{TableAccess, UserContext, effective_table_filter};
use crate::project::config::ProxyConfig;
use crate::proxy_project;
use crate::response::xml_escape;
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

        let cardinality = match &access {
            TableAccess::Filtered(sql) => {
                let sql_count = format!(
                    "SELECT COUNT(DISTINCT {}) FROM {} WHERE {}",
                    dim.physical_field, dim_table, sql
                );
                backend.query_count(&sql_count)
            }
            _ => {
                let sql = format!(
                    "SELECT COUNT(DISTINCT {}) FROM {}",
                    dim.physical_field, dim_table,
                );
                backend.query_count(&sql)
            }
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

        let sql = match &access {
            TableAccess::Filtered(sql_filter) => format!(
                "SELECT DISTINCT {} FROM {} WHERE {} ORDER BY {}",
                dim.physical_field, dim_table, sql_filter, dim.physical_field
            ),
            _ => format!(
                "SELECT DISTINCT {} FROM {} ORDER BY {}",
                dim.physical_field, dim_table, dim.physical_field
            ),
        };
        let values = backend.query_strings(&sql);
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
                member_unique_name: leaf_member_u,
                parent_unique_name: Some(all_member_u.clone()),
            });
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

/// Query children of a specific multi-level hierarchy member.
/// Parses filter like `[Date].[Date].[Year].&[2025]`, resolves the next
/// level's column, and queries DuckDB directly.  Returns None if the
/// filter doesn't match a multi-level dimension.
fn key_suffix(key: &str) -> String {
    key.split('|').map(|part| format!("&[{}]", part)).collect()
}

fn query_level_children<B: QueryBackend + ?Sized>(
    filter: &str,
    _existing_rows: &[MemberRow],
    backend: &B,
) -> Option<Vec<MemberRow>> {
    let decoded = filter.replace("&amp;", "&");
    // Parse [Hier].[Hier].[Level].&[key] — the key may be compound
    // (&[2026]&[4]) to carry the ancestor path for a non-unique level.
    let (hier_path, level_name, key) = parse_level_member(&decoded)?;
    let project = proxy_project::project();
    let model = &project.model;
    let dim = model
        .dimensions
        .iter()
        .find(|d| d.hierarchy_unique_name() == hier_path)?;
    let level_idx = dim.levels.iter().position(|l| l.name == level_name)?;
    let next_level = dim.levels.get(level_idx + 1)?;
    let table = model.dim_table_for_discovery(&dim.id);

    // Scope by the ancestor path carried in the key. A quarter's key 2026|4
    // must filter both year=2026 and quarter=4, not all years' Q4s. Align the
    // key to the end of the level chain (a bare key applies to the level it is
    // on).
    let key_parts: Vec<&str> = key.split('|').collect();
    let start = level_idx + 1 - key_parts.len();
    let conditions: Vec<String> = key_parts
        .iter()
        .enumerate()
        .filter_map(|(j, v)| {
            let l = dim.levels.get(start + j)?;
            Some(format!(
                "CAST({} AS VARCHAR) = '{}'",
                l.column,
                v.replace('\'', "''")
            ))
        })
        .collect();
    let where_sql = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };
    let count_sql = format!(
        "SELECT COUNT(DISTINCT {}) FROM {}{}",
        next_level.column, table, where_sql
    );
    let child_count = backend.query_count(&count_sql);
    // Per-child cardinality: the number of children each returned member has
    // (a quarter has 3 months, a month has its day count), not the global
    // next-next level count, which Excel sees as inconsistent with the cellset.
    let child_cc_map: std::collections::HashMap<String, u32> = match dim.levels.get(level_idx + 2) {
        Some(nn) => {
            let cc_sql = format!(
                "SELECT CAST({} AS VARCHAR), COUNT(DISTINCT {}) FROM {}{} GROUP BY 1",
                next_level.column, nn.column, table, where_sql
            );
            backend
                .query_grouped_1d(&cc_sql)
                .into_iter()
                .map(|(k, v)| (k, v as u32))
                .collect()
        }
        None => std::collections::HashMap::new(),
    };
    let sql = format!(
        "SELECT DISTINCT CAST({} AS VARCHAR) FROM {}{} ORDER BY 1",
        next_level.column, table, where_sql
    );
    let child_names = backend.query_strings(&sql);
    if child_names.is_empty() {
        return Some(vec![]);
    }
    let parent_u = format!("{}.[{}].{}", hier_path, level_name, key_suffix(&key));
    let mut rows: Vec<MemberRow> = child_names
        .iter()
        .enumerate()
        .map(|(ord, name)| {
            let child_key = format!("{key}|{name}");
            let child_u = format!(
                "{}.[{}].{}",
                hier_path,
                next_level.name,
                key_suffix(&child_key)
            );
            let child_level_u = format!("{}.[{}]", hier_path, next_level.name);
            let child_cc = child_cc_map.get(name).copied().unwrap_or(0);
            let xml = member_xml_for_discover(
                &dim.dimension_unique_name(),
                &dim.hierarchy_unique_name(),
                &child_level_u,
                (level_idx + 2) as u32,
                (ord + 1) as u32,
                name,
                &child_u,
                name,
                child_cc,
                (level_idx + 1) as u32,
                Some(&parent_u),
                name,
            );
            MemberRow {
                xml,
                dimension_id: dim.id.clone(),
                member_unique_name: child_u,
                parent_unique_name: Some(parent_u.clone()),
            }
        })
        .collect();
    // Prepend the parent member, whose own parent is the ancestor level (not
    // always the (All) member once a quarter carries a year).
    let parent_cardinality = child_count;
    let parent_level_u = format!("{}.[{}]", hier_path, level_name);
    let parent_name = key.rsplit('|').next().unwrap_or(&key).to_string();
    let parent_parent = if level_idx == 0 {
        dim.all_member_unique_name()
    } else {
        let ancestor_key = key_parts[..key_parts.len() - 1].join("|");
        format!(
            "{}.[{}].{}",
            hier_path,
            dim.levels[level_idx - 1].name,
            key_suffix(&ancestor_key)
        )
    };
    let parent_xml = member_xml_for_discover(
        &dim.dimension_unique_name(),
        &dim.hierarchy_unique_name(),
        &parent_level_u,
        (level_idx + 1) as u32,
        0,
        &parent_name,
        &parent_u,
        &parent_name,
        parent_cardinality,
        0,
        Some(&parent_parent),
        &parent_name,
    );
    rows.insert(
        0,
        MemberRow {
            xml: parent_xml,
            dimension_id: dim.id.clone(),
            member_unique_name: parent_u.clone(),
            parent_unique_name: Some(parent_parent),
        },
    );
    Some(rows)
}

/// Try to parse `[Hier].[Hier].[Level].&[key]` (key may be compound, e.g.
/// `&[2026]&[4]`) into (hier_path, level_name, key_path).
fn parse_level_member(filter: &str) -> Option<(String, String, String)> {
    let rest = filter.strip_prefix('[')?;
    let close = rest.find(']')?;
    let _dim = &rest[..close];
    let rest = &rest[close + 1..];
    let rest = rest.strip_prefix(".[")?;
    let close = rest.find(']')?;
    let hier_part = &rest[..close];
    let rest = &rest[close + 1..];
    let rest = rest.strip_prefix(".[")?;
    let close = rest.find(']')?;
    let level = &rest[..close];
    let rest = &rest[close + 1..];
    let mut rest = rest.strip_prefix(".&[")?;
    let mut parts = Vec::new();
    loop {
        let close = rest.find(']')?;
        parts.push(rest[..close].to_string());
        rest = &rest[close + 1..];
        if let Some(next) = rest.strip_prefix("&[") {
            rest = next;
        } else {
            break;
        }
    }
    let key = parts.join("|");
    let hier_path = format!("[{}].[{}]", _dim, hier_part);
    Some((hier_path, level.to_string(), key))
}

#[allow(clippy::too_many_arguments)] // XML row assembly mirrors the flat MDSCHEMA_MEMBERS column list
fn member_xml_for_discover(
    dim_u: &str,
    hier_u: &str,
    level_u: &str,
    level_num: u32,
    member_ordinal: u32,
    member_name: &str,
    member_unique_name: &str,
    member_caption: &str,
    children_cardinality: u32,
    parent_level: u32,
    parent_unique_name: Option<&str>,
    member_key: &str,
) -> String {
    let project = proxy_project::project();
    let pun = parent_unique_name
        .map(|p| {
            format!(
                "            <PARENT_UNIQUE_NAME>{}</PARENT_UNIQUE_NAME>\n",
                xml_escape(p)
            )
        })
        .unwrap_or_default();
    let parent_count = if parent_unique_name.is_some() { 1 } else { 0 };
    format!(
        r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_e}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier_e}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>{level_e}</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>{ln}</LEVEL_NUMBER>
            <MEMBER_ORDINAL>{mo}</MEMBER_ORDINAL>
            <MEMBER_NAME>{mn}</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>{mu}</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_GUID>{guid}</MEMBER_GUID>
            <MEMBER_CAPTION>{mc}</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>{cc}</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>{pl}</PARENT_LEVEL>{pun}
            <PARENT_COUNT>{pc}</PARENT_COUNT>
            <MEMBER_KEY>{mk}</MEMBER_KEY>
          </row>
"#,
        catalog = project.config.catalog,
        cube = project.config.cube,
        dim_e = xml_escape(dim_u),
        hier_e = xml_escape(hier_u),
        level_e = xml_escape(level_u),
        ln = level_num,
        mo = member_ordinal,
        mn = xml_escape(member_name),
        mu = xml_escape(member_unique_name),
        mc = xml_escape(member_caption),
        cc = children_cardinality,
        pl = parent_level,
        pc = parent_count,
        mk = xml_escape(member_key),
        guid = Uuid::new_v5(&Uuid::NAMESPACE_OID, member_unique_name.as_bytes()),
    )
}

fn all_rows_with_backend<B: QueryBackend + ?Sized>(
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> Vec<MemberRow> {
    let mut rows = all_member_rows_with_backend(backend, user, config);
    rows.append(&mut leaf_member_rows_with_backend(backend, user, config));
    rows
}

// ---- filter/search helpers (reimplemented over Vec<MemberRow>) ----

fn find_member<'a>(rows: &'a [MemberRow], filter: &str) -> Option<&'a MemberRow> {
    let decoded = filter.replace("&amp;", "&");
    rows.iter()
        .find(|r| r.member_unique_name == filter || r.member_unique_name == decoded)
}

fn find_children<'a>(rows: &'a [MemberRow], parent: &str) -> Vec<&'a MemberRow> {
    let decoded = parent.replace("&amp;", "&");
    rows.iter()
        .filter(|r| {
            r.parent_unique_name
                .as_deref()
                .is_some_and(|pun| pun == parent || pun == decoded)
        })
        .collect()
}

// ---- public API ----

pub fn get_members_response(member_filter: Option<&str>, tree_op: Option<i32>) -> String {
    let project = proxy_project::project();
    get_members_response_with_backend(
        member_filter,
        tree_op,
        Backend::get(),
        &UserContext::admin_default(),
        &project.config,
    )
}

pub fn get_members_response_with_backend<B: QueryBackend + ?Sized>(
    member_filter: Option<&str>,
    tree_op: Option<i32>,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> String {
    let mut rows = all_rows_with_backend(backend, user, config);

    let selected: Vec<&MemberRow> = match (member_filter, tree_op) {
        (Some(filter), Some(1) | Some(8)) => {
            if let Some(extra) = query_level_children(filter, &rows, backend) {
                rows.extend(extra);
            }
            // Now search in the extended rows
            let mut children = find_children(&rows, filter);
            if let Some(parent) = find_member(&rows, filter) {
                let mut result: Vec<&MemberRow> = vec![parent];
                result.append(&mut children);
                result
            } else {
                children
            }
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
            // No filter: return all members
            rows.iter().collect()
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

    fn all_rows() -> Vec<MemberRow> {
        let project = proxy_project::project();
        all_rows_with_backend(
            Backend::get(),
            &UserContext::admin_default(),
            &project.config,
        )
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
        assert!(dims.contains("Produktkategori"));
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
        assert!(xml.contains("[Produktkategori]"));
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
}
