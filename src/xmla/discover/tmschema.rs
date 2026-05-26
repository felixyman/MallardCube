use crate::response::discover_rowset_envelope;

/// Helper to build a TMSCHEMA_* envelope with one column declared (ID) and arbitrary rows.
fn tm_envelope(row_fields: &str, rows: &str) -> String {
    discover_rowset_envelope("", row_fields, rows)
}

const ID_ONLY_FIELDS: &str = r#"                <xsd:element sql:field="ID" name="ID" type="xsd:long" minOccurs="0"/>"#;

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

// -------- TMSCHEMA_TABLES: Faktatabell + Produktkategori --------
pub fn get_tmschema_tables_response() -> String {
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
    let rows = r#"          <row>
            <ID>2</ID>
            <ModelID>1</ModelID>
            <Name>Faktatabell</Name>
            <Description>Fact table</Description>
            <IsHidden>false</IsHidden>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
            <StructureModifiedTime>2026-05-20T12:00:00</StructureModifiedTime>
            <IsPrivate>false</IsPrivate>
            <ShowAsVariationsOnly>false</ShowAsVariationsOnly>
            <SystemFlags>0</SystemFlags>
          </row>
          <row>
            <ID>3</ID>
            <ModelID>1</ModelID>
            <Name>Produktkategori</Name>
            <Description>Product category dimension</Description>
            <IsHidden>false</IsHidden>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
            <StructureModifiedTime>2026-05-20T12:00:00</StructureModifiedTime>
            <IsPrivate>false</IsPrivate>
            <ShowAsVariationsOnly>false</ShowAsVariationsOnly>
            <SystemFlags>0</SystemFlags>
          </row>"#;
    tm_envelope(row_fields, rows)
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

// -------- TMSCHEMA_RELATIONSHIPS: 1 row (Faktatabell.ProductKey → Produktkategori.ProductKey) --------
pub fn get_tmschema_relationships_response() -> String {
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
    let rows = r#"          <row>
            <ID>10</ID>
            <ModelID>1</ModelID>
            <Name>Faktatabell_Produktkategori</Name>
            <FromTableID>2</FromTableID>
            <FromColumnID>20</FromColumnID>
            <FromCardinality>2</FromCardinality>
            <ToTableID>3</ToTableID>
            <ToColumnID>30</ToColumnID>
            <ToCardinality>1</ToCardinality>
            <IsActive>true</IsActive>
            <CrossFilteringBehavior>1</CrossFilteringBehavior>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
          </row>"#;
    tm_envelope(row_fields, rows)
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
