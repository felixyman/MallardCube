use crate::response::{discover_rowset_envelope, UUID_TYPE};

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

const MEMBER_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <MEMBER_ORDINAL>0</MEMBER_ORDINAL>
            <MEMBER_NAME>All</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Produktkategori].[All]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>2</MEMBER_TYPE>
            <MEMBER_GUID>00000000-0000-0000-0000-000000000100</MEMBER_GUID>
            <MEMBER_CAPTION>All</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>4</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>0</PARENT_COUNT>
            <MEMBER_KEY>All</MEMBER_KEY>
            <IS_PLACEHOLDERMEMBER>false</IS_PLACEHOLDERMEMBER>
            <IS_DATAMEMBER>false</IS_DATAMEMBER>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>1</MEMBER_ORDINAL>
            <MEMBER_NAME>Kategori A</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Produktkategori].&amp;[Kategori A]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_GUID>00000000-0000-0000-0000-000000000101</MEMBER_GUID>
            <MEMBER_CAPTION>Kategori A</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_UNIQUE_NAME>[Produktkategori].[Produktkategori].[All]</PARENT_UNIQUE_NAME>
            <PARENT_COUNT>1</PARENT_COUNT>
            <MEMBER_KEY>Kategori A</MEMBER_KEY>
            <IS_PLACEHOLDERMEMBER>false</IS_PLACEHOLDERMEMBER>
            <IS_DATAMEMBER>false</IS_DATAMEMBER>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>2</MEMBER_ORDINAL>
            <MEMBER_NAME>Kategori B</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Produktkategori].&amp;[Kategori B]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_GUID>00000000-0000-0000-0000-000000000102</MEMBER_GUID>
            <MEMBER_CAPTION>Kategori B</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_UNIQUE_NAME>[Produktkategori].[Produktkategori].[All]</PARENT_UNIQUE_NAME>
            <PARENT_COUNT>1</PARENT_COUNT>
            <MEMBER_KEY>Kategori B</MEMBER_KEY>
            <IS_PLACEHOLDERMEMBER>false</IS_PLACEHOLDERMEMBER>
            <IS_DATAMEMBER>false</IS_DATAMEMBER>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>3</MEMBER_ORDINAL>
            <MEMBER_NAME>Kategori C</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Produktkategori].&amp;[Kategori C]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_GUID>00000000-0000-0000-0000-000000000103</MEMBER_GUID>
            <MEMBER_CAPTION>Kategori C</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_UNIQUE_NAME>[Produktkategori].[Produktkategori].[All]</PARENT_UNIQUE_NAME>
            <PARENT_COUNT>1</PARENT_COUNT>
            <MEMBER_KEY>Kategori C</MEMBER_KEY>
            <IS_PLACEHOLDERMEMBER>false</IS_PLACEHOLDERMEMBER>
            <IS_DATAMEMBER>false</IS_DATAMEMBER>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>4</MEMBER_ORDINAL>
            <MEMBER_NAME>Kategori D</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Produktkategori].&amp;[Kategori D]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_GUID>00000000-0000-0000-0000-000000000104</MEMBER_GUID>
            <MEMBER_CAPTION>Kategori D</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_UNIQUE_NAME>[Produktkategori].[Produktkategori].[All]</PARENT_UNIQUE_NAME>
            <PARENT_COUNT>1</PARENT_COUNT>
            <MEMBER_KEY>Kategori D</MEMBER_KEY>
            <IS_PLACEHOLDERMEMBER>false</IS_PLACEHOLDERMEMBER>
            <IS_DATAMEMBER>false</IS_DATAMEMBER>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Region]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Region].[Region]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Region].[Region].[(All)]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <MEMBER_ORDINAL>0</MEMBER_ORDINAL>
            <MEMBER_NAME>All</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Region].[Region].[All]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>2</MEMBER_TYPE>
            <MEMBER_GUID>00000000-0000-0000-0000-000000000200</MEMBER_GUID>
            <MEMBER_CAPTION>All</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>2</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>0</PARENT_COUNT>
            <MEMBER_KEY>All</MEMBER_KEY>
            <IS_PLACEHOLDERMEMBER>false</IS_PLACEHOLDERMEMBER>
            <IS_DATAMEMBER>false</IS_DATAMEMBER>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Region]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Region].[Region]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Region].[Region].[Region]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>1</MEMBER_ORDINAL>
            <MEMBER_NAME>North</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Region].[Region].&amp;[North]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_GUID>00000000-0000-0000-0000-000000000201</MEMBER_GUID>
            <MEMBER_CAPTION>North</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_UNIQUE_NAME>[Region].[Region].[All]</PARENT_UNIQUE_NAME>
            <PARENT_COUNT>1</PARENT_COUNT>
            <MEMBER_KEY>North</MEMBER_KEY>
            <IS_PLACEHOLDERMEMBER>false</IS_PLACEHOLDERMEMBER>
            <IS_DATAMEMBER>false</IS_DATAMEMBER>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Region]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Region].[Region]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Region].[Region].[Region]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>2</MEMBER_ORDINAL>
            <MEMBER_NAME>South</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Region].[Region].&amp;[South]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_GUID>00000000-0000-0000-0000-000000000202</MEMBER_GUID>
            <MEMBER_CAPTION>South</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_UNIQUE_NAME>[Region].[Region].[All]</PARENT_UNIQUE_NAME>
            <PARENT_COUNT>1</PARENT_COUNT>
            <MEMBER_KEY>South</MEMBER_KEY>
            <IS_PLACEHOLDERMEMBER>false</IS_PLACEHOLDERMEMBER>
            <IS_DATAMEMBER>false</IS_DATAMEMBER>
          </row>"#;

// ---- filter helpers ----

/// Extract the text content of a named XML tag from a string.
/// Handles both `<tag>value</tag>` and `<tag/>` (self-closing → empty).
fn tag_content<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open_bare = format!("<{}", tag);
    let pos = xml.find(&open_bare)?;
    let after = &xml[pos..];
    if after.starts_with(&format!("<{}/>", tag)) || after.starts_with(&format!("<{} />", tag)) {
        return Some("");
    }
    let open_start = xml.find(&format!("<{}>", tag))? + tag.len() + 2;
    let end = xml[open_start..].find(&format!("</{}>", tag))? + open_start;
    Some(&xml[open_start..end])
}

/// Compare a filter value against the MEMBER_UNIQUE_NAME in a row,
/// handling XML entity encoding differences.  The filter arrives
/// unescaped (parser::unescape() decoded `&amp;` → `&`), while the
/// raw row string may still contain `&amp;`.
fn member_name_in_row(row: &str, filter: &str) -> bool {
    if let Some(name) = tag_content(row, "MEMBER_UNIQUE_NAME") {
        let decoded = name.replace("&amp;", "&").replace("&lt;", "<")
                         .replace("&gt;", ">").replace("&quot;", "\"")
                         .replace("&apos;", "'");
        decoded == filter || name == filter
    } else {
        false
    }
}

/// Search MEMBER_ROWS for a row whose `<MEMBER_UNIQUE_NAME>` matches `filter`.
fn find_member_row(filter: &str) -> Option<&'static str> {
    let rest = MEMBER_ROWS;
    let mut pos = 0;
    while pos < rest.len() {
        let row_start = match rest[pos..].find("<row>") {
            Some(i) => pos + i,
            None => break,
        };
        let row_end = match rest[row_start..].find("</row>") {
            Some(i) => row_start + i + 6,
            None => break,
        };
        let row = &rest[row_start..row_end];
        if member_name_in_row(row, filter) {
            return Some(row);
        }
        pos = row_end;
    }
    None
}

/// Search MEMBER_ROWS for rows that are children of `parent`.
/// Uses PARENT_UNIQUE_NAME for accurate parent-child matching.
fn find_children_of(parent: &str) -> Vec<&'static str> {
    if let Some(parent_row) = find_member_row(parent) {
        if let Some(cc) = tag_content(parent_row, "CHILDREN_CARDINALITY") {
            if cc.trim() == "0" {
                return vec![];
            }
        }
    } else {
        return vec![];
    }

    let rest = MEMBER_ROWS;
    let mut result = vec![];
    let mut pos = 0;
    while pos < rest.len() {
        let row_start = match rest[pos..].find("<row>") {
            Some(i) => pos + i,
            None => break,
        };
        let row_end = match rest[row_start..].find("</row>") {
            Some(i) => row_start + i + 6,
            None => break,
        };
        let row = &rest[row_start..row_end];
        if let Some(pun) = tag_content(row, "PARENT_UNIQUE_NAME") {
            let decoded = pun.replace("&amp;", "&");
            if decoded == parent || pun == parent {
                result.push(row);
            }
        }
        pos = row_end;
    }
    result
}

// ---- public API ----

pub fn get_members_response(member_filter: Option<&str>, tree_op: Option<i32>) -> String {
    let rows = match (member_filter, tree_op) {
        (Some(filter), Some(1)) => {
            // 0x01 = CHILDREN
            find_children_of(filter).join("\n")
        }
        (Some(filter), Some(4)) => {
            // 0x04 = PARENT — for a leaf member, return its parent
            if let Some(leaf) = find_member_row(filter) {
                if let Some(pun) = tag_content(leaf, "PARENT_UNIQUE_NAME") {
                    if !pun.is_empty() {
                        find_member_row(pun)
                            .map(|r| r.to_string())
                            .unwrap_or_default()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        }
        (Some(filter), Some(8)) => {
            // 0x08 = SELF — return the matching member
            find_member_row(filter)
                .map(|r| r.to_string())
                .unwrap_or_default()
        }
        (Some(filter), Some(32)) => {
            // 0x20 = ANCESTORS — return parent chain up to All.
            if let Some(row) = find_member_row(filter) {
                if row.contains("<PARENT_COUNT>0</PARENT_COUNT>") {
                    String::new()
                } else if let Some(pun) = tag_content(row, "PARENT_UNIQUE_NAME") {
                    find_member_row(pun)
                        .map(|r| r.to_string())
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        }
        (Some(filter), _) => {
            // Unknown TREE_OP — return the matching member
            find_member_row(filter)
                .map(|r| r.to_string())
                .unwrap_or_default()
        }
        (None, _) => {
            MEMBER_ROWS.to_string()
        }
    };

    discover_rowset_envelope(UUID_TYPE, MEMBER_ROW_FIELDS, &rows)
}
