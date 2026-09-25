use crate::engine::model::{SemanticModel, UserContext};
use crate::project::config::ProxyConfig;
use crate::response::discover_rowset_envelope;

/// Visible tables with stable ids, in the order TMSCHEMA_TABLES lists them:
/// fact tables first, then each dimension's discovery table once. Shared with
/// TMSCHEMA_RELATIONSHIPS so both rowsets agree on the identifiers.
/// `(id, table name, is a date table)` rows plus a name-to-id map for the
/// relationship rowset.
type VisibleTables = (
    Vec<(u32, String, bool)>,
    std::collections::BTreeMap<String, u32>,
);

fn visible_tables(
    model: &SemanticModel,
    user: &UserContext,
    config: &ProxyConfig,
) -> VisibleTables {
    let mut rows: Vec<(u32, String, bool)> = Vec::new();
    let mut ids: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let mut id = 10u32;
    for ft in &model.fact_tables {
        if !super::table_visible(config, user, &ft.table_name) || ids.contains_key(&ft.table_name) {
            continue;
        }
        ids.insert(ft.table_name.clone(), id);
        rows.push((id, ft.table_name.clone(), false));
        id += 1;
    }
    for d in &model.dimensions {
        let table = model.dim_table_for_discovery(&d.id).to_string();
        if !super::table_visible(config, user, &table) || ids.contains_key(&table) {
            continue;
        }
        ids.insert(table.clone(), id);
        rows.push((id, table, d.is_date_role));
        id += 1;
    }
    (rows, ids)
}

/// Helper to build a TMSCHEMA_* envelope with one column declared (ID) and arbitrary rows.
fn tm_envelope(row_fields: &str, rows: &str) -> String {
    discover_rowset_envelope("", row_fields, rows)
}

const ID_ONLY_FIELDS: &str =
    r#"                <xsd:element sql:field="ID" name="ID" type="xsd:long" minOccurs="0"/>"#;

// -------- TMSCHEMA_MODEL: 1 row --------
pub fn get_tmschema_model_response() -> String {
    let row_fields = r#"                <xsd:element sql:field="ID" name="ID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="Description" name="Description" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="Culture" name="Culture" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="ModifiedTime" name="ModifiedTime" type="xsd:dateTime" minOccurs="0"/>"#;
    let rows = r#"          <row>
            <ID>1</ID>
            <Name>Model</Name>
            <Description>Tabular model exposed by Rust XMLA proxy</Description>
            <Culture>sv-SE</Culture>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
          </row>"#;
    tm_envelope(row_fields, rows)
}

// -------- TMSCHEMA_TABLES: the model's own tables --------
pub fn get_tmschema_tables_response(user: &UserContext, config: &ProxyConfig) -> String {
    let row_fields = r#"                <xsd:element sql:field="ID" name="ID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ModelID" name="ModelID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DataCategory" name="DataCategory" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="Description" name="Description" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IsHidden" name="IsHidden" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="TableStorageID" name="TableStorageID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ModifiedTime" name="ModifiedTime" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="StructureModifiedTime" name="StructureModifiedTime" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="IsPrivate" name="IsPrivate" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="ShowAsVariationsOnly" name="ShowAsVariationsOnly" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="SystemFlags" name="SystemFlags" type="xsd:long" minOccurs="0"/>"#;
    let project = crate::proxy_project::project();
    let (tables, _) = visible_tables(&project.model, user, config);
    let mut rows = String::new();
    for (id, name, is_date) in tables {
        let data_category = if is_date { "Time" } else { "" };
        rows.push_str(&format!(
            r#"          <row>
            <ID>{id}</ID>
            <ModelID>1</ModelID>
            <Name>{name}</Name>
            <DataCategory>{data_category}</DataCategory>
            <Description></Description>
            <IsHidden>false</IsHidden>
            <TableStorageID>{storage}</TableStorageID>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
            <StructureModifiedTime>2026-05-20T12:00:00</StructureModifiedTime>
            <IsPrivate>false</IsPrivate>
            <ShowAsVariationsOnly>false</ShowAsVariationsOnly>
            <SystemFlags>0</SystemFlags>
          </row>
"#,
            storage = 80 + id,
        ));
    }
    tm_envelope(row_fields, &rows)
}

// -------- TMSCHEMA_COLUMNS: empty stub --------
pub fn get_tmschema_columns_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- TMSCHEMA_MEASURES: empty stub (real measure lives in MDSCHEMA_MEASURES) --------
pub fn get_tmschema_measures_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- TMSCHEMA_HIERARCHIES: empty stub --------
pub fn get_tmschema_hierarchies_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- TMSCHEMA_LEVELS: empty stub --------
pub fn get_tmschema_levels_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- TMSCHEMA_RELATIONSHIPS: the model's own relationships --------
pub fn get_tmschema_relationships_response(user: &UserContext, config: &ProxyConfig) -> String {
    let row_fields = r#"                <xsd:element sql:field="ID" name="ID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ModelID" name="ModelID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="FromTableID" name="FromTableID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="FromColumnID" name="FromColumnID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="FromCardinality" name="FromCardinality" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ToTableID" name="ToTableID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ToColumnID" name="ToColumnID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ToCardinality" name="ToCardinality" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="IsActive" name="IsActive" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="CrossFilteringBehavior" name="CrossFilteringBehavior" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ModifiedTime" name="ModifiedTime" type="xsd:dateTime" minOccurs="0"/>"#;
    let project = crate::proxy_project::project();
    let model = &project.model;
    let (_, ids) = visible_tables(model, user, config);
    let mut rows = String::new();
    let mut id = 100u32;
    for rel in &model.relationships {
        // A relationship whose fact or dimension table is hidden is not
        // advertised either.
        let Some(fact_table) = model
            .fact_tables
            .iter()
            .find(|ft| ft.id == rel.fact_table_id)
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) =
            (ids.get(&fact_table.table_name), ids.get(&rel.dim_table))
        else {
            continue;
        };
        rows.push_str(&format!(
            r#"          <row>
            <ID>{id}</ID>
            <ModelID>1</ModelID>
            <Name>{from}_{to}</Name>
            <FromTableID>{from_id}</FromTableID>
            <FromCardinality>2</FromCardinality>
            <ToTableID>{to_id}</ToTableID>
            <ToCardinality>1</ToCardinality>
            <IsActive>true</IsActive>
            <CrossFilteringBehavior>1</CrossFilteringBehavior>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
          </row>
"#,
            from = fact_table.table_name,
            to = rel.dim_table,
        ));
        id += 1;
    }
    tm_envelope(row_fields, &rows)
}

// -------- TMSCHEMA_PARTITIONS: empty stub --------
pub fn get_tmschema_partitions_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- DISCOVER_XML_METADATA: empty rowset stub --------
pub fn get_discover_xml_metadata_response() -> String {
    let row_fields = r#"                <xsd:element sql:field="METADATA" name="METADATA" type="xsd:string" minOccurs="0"/>"#;
    tm_envelope(row_fields, "")
}

// -------- DISCOVER_CALC_DEPENDENCY: empty stub --------
pub fn get_discover_calc_dependency_response() -> String {
    let row_fields = r#"                <xsd:element sql:field="DATABASE_NAME" name="DATABASE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="OBJECT_TYPE" name="OBJECT_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE" name="TABLE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="OBJECT" name="OBJECT" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="REFERENCED_OBJECT_TYPE" name="REFERENCED_OBJECT_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="REFERENCED_TABLE" name="REFERENCED_TABLE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="REFERENCED_OBJECT" name="REFERENCED_OBJECT" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="REFERENCED_EXPRESSION" name="REFERENCED_EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="QUERY" name="QUERY" type="xsd:string" minOccurs="0"/>"#;
    tm_envelope(row_fields, "")
}
