//! `MDSCHEMA_FUNCTIONS` — the MDX function list Excel reads when it opens the
//! "MDX Calculated Member" dialog (plan 049).
//!
//! Excel asks for `<ORIGIN>1</ORIGIN>` (built-in MDX functions) as soon as that
//! dialog opens. The proxy used to answer with an empty body, which Excel
//! reported as *"XML parsing failed at line 1, column 0: A document must
//! contain exactly one root element"*. The schema below is the reference's
//! (SSAS 2025 tabular) field for field, including the nested `PARAMETERINFO`
//! element — Excel validates the rows against it, so a field marked required
//! that a row omits is rejected outright.
//!
//! The function list is a curated subset of the reference's 154 rows: the MDX
//! surface the proxy lowers plus the common set/navigation/statistical/time
//! functions, so the dialog can offer and insert them. `ORIGIN=2`
//! (user-defined functions) is empty, as it is on the reference.

use crate::response::discover_rowset_envelope;
use crate::xmla::parser::Restrictions;

/// The reference's `MDSCHEMA_FUNCTIONS` row schema (all fields optional).
const FUNCTION_ROW_FIELDS: &str = r#"                <xsd:element sql:field="FUNCTION_NAME" name="FUNCTION_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PARAMETER_LIST" name="PARAMETER_LIST" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="RETURN_TYPE" name="RETURN_TYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="ORIGIN" name="ORIGIN" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="INTERFACE_NAME" name="INTERFACE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LIBRARY_NAME" name="LIBRARY_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DLL_NAME" name="DLL_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="HELP_FILE" name="HELP_FILE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="HELP_CONTEXT" name="HELP_CONTEXT" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="OBJECT" name="OBJECT" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CAPTION" name="CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PARAMETERINFO" name="PARAMETERINFO" minOccurs="0" maxOccurs="unbounded">
                  <xsd:complexType>
                    <xsd:sequence>
                      <xsd:element sql:field="NAME" name="NAME" type="xsd:string" minOccurs="0"/>
                      <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                      <xsd:element sql:field="OPTIONAL" name="OPTIONAL" type="xsd:boolean" minOccurs="0"/>
                      <xsd:element sql:field="REPEATABLE" name="REPEATABLE" type="xsd:boolean" minOccurs="0"/>
                      <xsd:element sql:field="REPEATGROUP" name="REPEATGROUP" type="xsd:int" minOccurs="0"/>
                      <xsd:element sql:field="SKIPPABLE" name="SKIPPABLE" type="xsd:boolean" minOccurs="0"/>
                    </xsd:sequence>
                  </xsd:complexType>
                </xsd:element>
                <xsd:element sql:field="DIRECTQUERY_PUSHABLE" name="DIRECTQUERY_PUSHABLE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="VISUAL_CALCULATIONS_INFO" name="VISUAL_CALCULATIONS_INFO" type="xsd:int" minOccurs="0"/>"#;

/// `(function name, interface)` — the reference's names and interfaces, sorted
/// as the reference sorts them (by name, then interface).
const FUNCTIONS: &[(&str, &str)] = &[
    ("ADDCALCULATEDMEMBERS", "Set"),
    ("AGGREGATE", "Statistical"),
    ("ALLMEMBERS", "Set"),
    ("ANCESTOR", "Navigation"),
    ("ANCESTORS", "Navigation"),
    ("ASCENDANTS", "Navigation"),
    ("AVG", "Statistical"),
    ("AXIS", "Metadata"),
    ("BOTTOMCOUNT", "Set"),
    ("BOTTOMPERCENT", "Set"),
    ("BOTTOMSUM", "Set"),
    ("CHILDREN", "Navigation"),
    ("CLOSINGPERIOD", "Time"),
    ("COALESCEEMPTY", "Statistical"),
    ("CORRELATION", "Statistical"),
    ("COUNT", "Statistical"),
    ("COUSIN", "Navigation"),
    ("COVARIANCE", "Statistical"),
    ("COVARIANCEN", "Statistical"),
    ("CROSSJOIN", "Set"),
    ("CURRENT", "Navigation"),
    ("CURRENTMEMBER", "Navigation"),
    ("CURRENTORDINAL", "Navigation"),
    ("CUSTOMDATA", "Other"),
    ("DATAMEMBER", "Navigation"),
    ("DEFAULTMEMBER", "Navigation"),
    ("DESCENDANTS", "Set"),
    ("DISTINCT", "Set"),
    ("DISTINCTCOUNT", "Statistical"),
    ("DRILLDOWNLEVEL", "UI"),
    ("DRILLDOWNLEVELBOTTOM", "UI"),
    ("DRILLDOWNLEVELTOP", "UI"),
    ("DRILLDOWNMEMBER", "UI"),
    ("DRILLDOWNMEMBERBOTTOM", "UI"),
    ("DRILLDOWNMEMBERTOP", "UI"),
    ("DRILLUPLEVEL", "UI"),
    ("DRILLUPMEMBER", "UI"),
    ("EXCEPT", "Set"),
    ("EXISTING", "Set"),
    ("EXISTS", "Set"),
    ("EXTRACT", "Set"),
    ("FILTER", "Set"),
    ("FIRSTCHILD", "Navigation"),
    ("FIRSTSIBLING", "Navigation"),
    ("GENERATE", "Set"),
    ("HEAD", "Set"),
    ("HIERARCHIZE", "Set"),
    ("HIERARCHY", "Metadata"),
    ("IIF", "Value"),
    ("INTERSECT", "Set"),
    ("IS", "Value"),
    ("ISANCESTOR", "Navigation"),
    ("ISEMPTY", "Value"),
    ("ISGENERATION", "Navigation"),
    ("ISLEAF", "Navigation"),
    ("ISSIBLING", "Navigation"),
    ("ITEM", "Other"),
    ("LAG", "Navigation"),
    ("LASTCHILD", "Navigation"),
    ("LASTPERIODS", "Time"),
    ("LASTSIBLING", "Navigation"),
    ("LEAD", "Navigation"),
    ("LEVEL", "Metadata"),
    ("LEVELS", "Metadata"),
    ("MAX", "Statistical"),
    ("MEASUREGROUPMEASURES", "Set"),
    ("MEDIAN", "Statistical"),
    ("MEMBERS", "Set"),
    ("MEMBERTOSTR", "String"),
    ("MEMBERVALUE", "Value"),
    ("MIN", "Statistical"),
    ("MTD", "Time"),
    ("NAME", "Metadata"),
    ("NAMETOSET", "String"),
    ("NEXTMEMBER", "Navigation"),
    ("NONEMPTY", "Set"),
    ("NONEMPTYCROSSJOIN", "Set"),
    ("OPENINGPERIOD", "Time"),
    ("ORDER", "Set"),
    ("ORDINAL", "Metadata"),
    ("PARALLELPERIOD", "Time"),
    ("PARENT", "Navigation"),
    ("PERIODSTODATE", "Time"),
    ("PREVMEMBER", "Navigation"),
    ("PROPERTIES", "Navigation"),
    ("QTD", "Time"),
    ("RANK", "Statistical"),
    ("SETTOSTR", "String"),
    ("SIBLINGS", "Navigation"),
    ("STDDEV", "Statistical"),
    ("STDEVP", "Statistical"),
    ("STRIPCALCULATEDMEMBERS", "Set"),
    ("STRTOMEMBER", "String"),
    ("STRTOSET", "String"),
    ("STRTOTUPLE", "String"),
    ("STRTOVALUE", "String"),
    ("SUBSET", "Set"),
    ("SUM", "Statistical"),
    ("TAIL", "Set"),
    ("TOGGLEDRILLSTATE", "UI"),
    ("TOPCOUNT", "Set"),
    ("TOPPERCENT", "Set"),
    ("TOPSUM", "Set"),
    ("TUPLETOSTR", "String"),
    ("UNION", "Set"),
    ("UNIQUENAME", "Metadata"),
    ("UNKNOWNMEMBER", "Navigation"),
    ("UNORDER", "Set"),
    ("VALIDMEASURE", "Value"),
    ("VALUE", "Value"),
    ("VAR", "Statistical"),
    ("VARIANCE", "Statistical"),
    ("VARIANCEP", "Statistical"),
    ("VARP", "Statistical"),
    ("WTD", "Time"),
    ("YTD", "Time"),
];

/// The `MDSCHEMA_FUNCTIONS` response. `ORIGIN=1` is the built-in MDX list;
/// anything else (user-defined functions) is empty, as on the reference.
pub fn get_functions_response(restrictions: &Restrictions) -> String {
    let origin = restrictions.origin.unwrap_or(1);
    let mut rows = String::new();
    if origin == 1 {
        for (name, interface) in FUNCTIONS {
            rows.push_str(&format!(
                r#"          <row>
            <FUNCTION_NAME>{name}</FUNCTION_NAME>
            <RETURN_TYPE>12</RETURN_TYPE>
            <ORIGIN>1</ORIGIN>
            <INTERFACE_NAME>{interface}</INTERFACE_NAME>
            <CAPTION>{name}</CAPTION>
          </row>
"#,
            ));
        }
    }
    discover_rowset_envelope("", FUNCTION_ROW_FIELDS, &rows)
}

#[cfg(test)]
mod tests {
    use crate::project::project::ProxyProject;
    use crate::project::project::with_test_project;
    use crate::xmla::parser::Restrictions;

    #[test]
    fn mdx_functions_are_listed_with_the_reference_schema() {
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let restrictions = Restrictions {
                origin: Some(1),
                ..Restrictions::default()
            };
            let resp = super::get_functions_response(&restrictions);
            // Excel reads the rows against this schema: every field optional.
            assert!(resp.contains(r#"name="FUNCTION_NAME" type="xsd:string" minOccurs="0""#), "{resp}");
            assert!(resp.contains(r#"name="RETURN_TYPE" type="xsd:int" minOccurs="0""#), "{resp}");
            for name in ["CROSSJOIN", "DRILLDOWNMEMBER", "YTD", "STRTOSET"] {
                assert!(
                    resp.contains(&format!("<FUNCTION_NAME>{name}</FUNCTION_NAME>")),
                    "{name} missing: {resp}"
                );
            }
            assert!(resp.contains("<ORIGIN>1</ORIGIN>"), "{resp}");
        });
    }

    #[test]
    fn user_defined_functions_are_empty() {
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let restrictions = Restrictions {
                origin: Some(2),
                ..Restrictions::default()
            };
            let resp = super::get_functions_response(&restrictions);
            assert!(!resp.contains("<row>"), "user-defined list must be empty: {resp}");
        });
    }
}
