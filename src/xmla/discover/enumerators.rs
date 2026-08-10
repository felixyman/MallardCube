/// DISCOVER_ENUMERATORS — returns the list of schema rowset names
/// that this server supports. Required by Excel CUBE functions to
/// validate the connection as a valid XMLA provider.
use crate::response::discover_rowset_envelope;

const ENUMERATOR_FIELDS: &str = r#"                <xsd:element sql:field="EnumName" name="EnumName" type="xsd:string"/>
                <xsd:element sql:field="EnumDescription" name="EnumDescription" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EnumType" name="EnumType" type="xsd:unsignedInt"/>
                <xsd:element sql:field="ElementName" name="ElementName" type="xsd:string" minOccurs="0"/>"#;

pub fn get_enumerators_response() -> String {
    let rows = [
        ("DBSCHEMA_CATALOGS", "Catalog schema", "1"),
        ("DBSCHEMA_TABLES", "Table schema", "1"),
        ("MDSCHEMA_CUBES", "Cube schema", "1"),
        ("MDSCHEMA_DIMENSIONS", "Dimension schema", "1"),
        ("MDSCHEMA_HIERARCHIES", "Hierarchy schema", "1"),
        ("MDSCHEMA_LEVELS", "Level schema", "1"),
        ("MDSCHEMA_MEASURES", "Measure schema", "1"),
        ("MDSCHEMA_PROPERTIES", "Property schema", "1"),
        ("MDSCHEMA_MEMBERS", "Member schema", "1"),
        ("MDSCHEMA_KPIS", "KPI schema", "1"),
        ("MDSCHEMA_SETS", "Set schema", "1"),
        ("MDSCHEMA_MEASUREGROUPS", "Measure group schema", "1"),
        (
            "MDSCHEMA_MEASUREGROUP_DIMENSIONS",
            "Measure group dimensions schema",
            "1",
        ),
        ("DISCOVER_SCHEMA_ROWSETS", "Schema rowsets", "1"),
        ("DISCOVER_ENUMERATORS", "Enumerators", "2"),
        ("DISCOVER_PROPERTIES", "Properties", "2"),
        ("DISCOVER_LITERALS", "Literals", "2"),
        ("DISCOVER_KEYWORDS", "Keywords", "2"),
        ("DISCOVER_DATASOURCES", "Data sources", "2"),
        ("DISCOVER_XML_METADATA", "XML metadata", "2"),
        ("DISCOVER_CALC_DEPENDENCY", "Calc dependency", "2"),
    ];

    let xml_rows: String = rows
        .iter()
        .map(|(name, desc, etype)| {
            format!(
                r#"          <row>
            <EnumName>{name}</EnumName>
            <EnumDescription>{desc}</EnumDescription>
            <EnumType>{etype}</EnumType>
          </row>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    discover_rowset_envelope("", ENUMERATOR_FIELDS, &xml_rows)
}
