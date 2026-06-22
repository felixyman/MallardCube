/// MDSCHEMA_MEMBERS rowset — responds to Excel's member discovery.
///
/// Member rows are generated from actual DuckDB data (distinct dimension
/// values) plus synthetic `All` members from the semantic model.
/// No hardcoded business values remain.

use crate::backend::{Backend, QueryBackend};
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
    dimension_id: String,
    member_unique_name: String,
    parent_unique_name: Option<String>,
    children_cardinality: u32,
}

fn build_all_member_rows<B: QueryBackend + ?Sized>(
    model: &crate::engine::model::SemanticModel,
    backend: &B,
) -> Vec<MemberRow> {
    let project = proxy_project::project();
    let mut rows = Vec::new();
    for dim in &model.dimensions {
        let dim_u = dim.dimension_unique_name();
        let hier_u = dim.hierarchy_unique_name();
        let all_level_u = dim.all_level_unique_name();
        let all_member_u = dim.all_member_unique_name();

        let sql = format!(
            "SELECT COUNT(DISTINCT {}) FROM {}",
            dim.physical_field,
            model.dim_table_for_discovery(&dim.id)
        );
        let cardinality = backend.query_count(&sql);
        let guid = all_member_guid(&dim.id);
        rows.push(MemberRow {
            xml: xml_member_row(
                project,
                &dim_u, &hier_u, &all_level_u,
                0, 0,
                "All", &all_member_u,
                2, &guid,
                "All", cardinality, 0, None, 0,
                "All",
            ),
            dimension_id: dim.id.clone(),
            member_unique_name: all_member_u,
            parent_unique_name: None,
            children_cardinality: cardinality,
        });
    }
    rows
}

fn build_leaf_member_rows<B: QueryBackend + ?Sized>(
    model: &crate::engine::model::SemanticModel,
    backend: &B,
) -> Vec<MemberRow> {
    let project = proxy_project::project();
    let mut rows = Vec::new();
    for dim in &model.dimensions {
        let dim_u = dim.dimension_unique_name();
        let hier_u = dim.hierarchy_unique_name();
        let leaf_level_u = dim.leaf_level_unique_name();
        let all_member_u = dim.all_member_unique_name();

        let sql = format!(
            "SELECT DISTINCT {} FROM {} ORDER BY {}",
            dim.physical_field,
            model.dim_table_for_discovery(&dim.id),
            dim.physical_field
        );
        let values = backend.query_strings(&sql);
        for (ordinal, val) in values.iter().enumerate() {
            let ordinal = ordinal as u32 + 1;
            let leaf_member_u = format!("{}.&[{}]", hier_u, val);
            let member_guid = leaf_member_guid(&dim.id, val);
            rows.push(MemberRow {
                xml: xml_member_row(
                    project,
                    &dim_u, &hier_u, &leaf_level_u,
                    1, ordinal,
                    val, &leaf_member_u,
                    1, &member_guid,
                    val, 0, 0,
                    Some(&all_member_u), 1,
                    val,
                ),
                dimension_id: dim.id.clone(),
                member_unique_name: leaf_member_u,
                parent_unique_name: Some(all_member_u.clone()),
                children_cardinality: 0,
            });
        }
    }
    rows
}

/// Stable namespace for v5 UUIDs.  Derived from "ssas-proxy" so every
/// member GUID is deterministic across runs but unique to this proxy.
const NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1,
    0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

fn all_member_guid(dim_id: &str) -> String {
    Uuid::new_v5(&NAMESPACE, format!("all.{dim_id}").as_bytes()).to_string()
}

fn leaf_member_guid(dim_id: &str, member_value: &str) -> String {
    Uuid::new_v5(&NAMESPACE, format!("leaf.{dim_id}.{member_value}").as_bytes()).to_string()
}

fn xml_member_row(
    project: &crate::proxy_project::ProxyProject,
    dim_u: &str, hier_u: &str, level_u: &str,
    level_num: u32, member_ordinal: u32,
    member_name: &str, member_unique_name: &str,
    member_type: u32, member_guid: &str,
    member_caption: &str,
    children_cardinality: u32,
    parent_level: u32,
    parent_unique_name: Option<&str>,
    parent_count: u32,
    member_key: &str,
) -> String {
    let pun = parent_unique_name
        .map(|p| format!("            <PARENT_UNIQUE_NAME>{}</PARENT_UNIQUE_NAME>\n", xml_escape(p)))
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

fn all_member_rows_with_backend<B: QueryBackend + ?Sized>(backend: &B) -> Vec<MemberRow> {
    let project = proxy_project::project();
    build_all_member_rows(&project.model, backend)
}

fn leaf_member_rows_with_backend<B: QueryBackend + ?Sized>(backend: &B) -> Vec<MemberRow> {
    let project = proxy_project::project();
    build_leaf_member_rows(&project.model, backend)
}

fn all_rows_with_backend<B: QueryBackend + ?Sized>(backend: &B) -> Vec<MemberRow> {
    let mut rows = all_member_rows_with_backend(backend);
    rows.append(&mut leaf_member_rows_with_backend(backend));
    rows
}

fn all_rows() -> Vec<MemberRow> {
    all_rows_with_backend(Backend::get())
}

// ---- filter/search helpers (reimplemented over Vec<MemberRow>) ----

fn find_member<'a>(rows: &'a [MemberRow], filter: &str) -> Option<&'a MemberRow> {
    let decoded = filter.replace("&amp;", "&");
    rows.iter().find(|r| {
        r.member_unique_name == filter || r.member_unique_name == decoded
    })
}

fn find_children<'a>(rows: &'a [MemberRow], parent: &str) -> Vec<&'a MemberRow> {
    let decoded = parent.replace("&amp;", "&");
    rows.iter().filter(|r| {
        r.parent_unique_name.as_deref()
            .map_or(false, |pun| pun == parent || pun == decoded)
    }).collect()
}

// ---- public API ----

pub fn get_members_response(member_filter: Option<&str>, tree_op: Option<i32>) -> String {
    get_members_response_with_backend(member_filter, tree_op, Backend::get())
}

pub fn get_members_response_with_backend<B: QueryBackend + ?Sized>(
    member_filter: Option<&str>,
    tree_op: Option<i32>,
    backend: &B,
) -> String {
    let rows = all_rows_with_backend(backend);

    let selected: Vec<&MemberRow> = match (member_filter, tree_op) {
        (Some(filter), Some(1)) => {
            // 0x01 = CHILDREN — return children of the filtered member
            let mut children = find_children(&rows, filter);
            // Also include the parent member itself before its children
            if let Some(parent) = find_member(&rows, filter) {
                let mut result = vec![parent];
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

    let xml_rows: String = selected.iter()
        .map(|r| r.xml.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    crate::response::discover_rowset_envelope("", MEMBER_ROW_FIELDS, &xml_rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_rows_for_both_dims() {
        let rows = all_rows();
        let dims: std::collections::HashSet<&str> = rows.iter()
            .map(|r| r.dimension_id.as_str())
            .collect();
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
        let leaf: Vec<_> = rows.iter()
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

fn extract_tag<'a>(xml: &'a str, tag: &str) -> Option<String> {
    let open = xml.find(&format!("<{tag}>"))? + tag.len() + 2;
    let close = xml[open..].find(&format!("</{tag}>"))?;
    Some(xml[open..open + close].to_string())
}

fn is_valid_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 { return false; }
    let lens = [8usize, 4, 4, 4, 12];
    for (i, p) in parts.iter().enumerate() {
        if p.len() != lens[i] { return false; }
        if !p.chars().all(|c| c.is_ascii_hexdigit()) { return false; }
    }
    true
}
