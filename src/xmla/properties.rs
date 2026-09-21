use crate::proxy_project;
use crate::response::{UUID_TYPE, discover_rowset_envelope};

struct Property {
    name: &'static str,
    description: &'static str,
    prop_type: &'static str,
    access_type: &'static str,
    is_required: bool,
    value: Option<&'static str>,
}

/// Read-only integer capability property (MDX/OLE DB negotiation). Excel asks
/// for these by name at connect time; a missing answer can make it withhold
/// member-bearing fields. Values match the reference SSAS 2025 and the
/// documented defaults ("Supported XMLA Properties",
/// learn.microsoft.com/analysis-services).
const fn int_capability(name: &'static str, value: &'static str) -> Property {
    Property {
        name,
        description: name,
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some(value),
    }
}

const PROPERTIES: &[Property] = &[
    Property {
        name: "ProviderName",
        description: "ProviderName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Min Riktiga Rust Proxy"),
    },
    Property {
        name: "DbpropMsmdSubqueries",
        description: "DbpropMsmdSubqueries",
        prop_type: "int",
        access_type: "ReadWrite",
        is_required: false,
        // 0x1 = FROM-clause subselects (applied as slicer restrictions),
        // 0x2 = WHERE-clause subqueries. Excel only enables
        // attribute/level drilling when the lowest two bits are set.
        value: Some("3"),
    },
    Property {
        name: "DbpropMsmdOptimizeResponse",
        description: "DbpropMsmdOptimizeResponse",
        prop_type: "long",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("9"),
    },
    Property {
        name: "DbpropMsmdActivityID",
        description: "DbpropMsmdActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "DbpropMsmdCurrentActivityID",
        description: "DbpropMsmdCurrentActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "ApplicationContext",
        description: "ApplicationContext",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "Catalog",
        description: "Catalog",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "ServerName",
        description: "ServerName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("rust-proxy"),
    },
    Property {
        name: "ProviderVersion",
        description: "ProviderVersion",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("16.0.0.0"),
    },
    Property {
        name: "MdpropMdxSubqueries",
        description: "MdpropMdxSubqueries",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("63"),
    },
    Property {
        name: "MdpropMdxDrillFunctions",
        description: "MdpropMdxDrillFunctions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("7"),
    },
    Property {
        name: "MdpropMdxNamedSets",
        description: "MdpropMdxNamedSets",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        // Named sets (`WITH SET` / `CREATE SET`) are not implemented yet
        // (plan 046). SSAS returns a capability bitmask; 0 is honest until
        // the MDX support lands — otherwise Excel offers "Manage Sets" and
        // sends MDX the proxy cannot execute.
        value: Some("0"),
    },
    Property {
        name: "MdpropMdxDdlExtensions",
        description: "MdpropMdxDdlExtensions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("23"),
    },
    Property {
        name: "MDXSupport",
        description: "MDXSupport",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Core"),
    },
    // MDX capability bitmasks Excel negotiates at connect time (see
    // `int_capability` above).
    int_capability("ProviderType", "6"),
    int_capability("MdpropMdxCaseSupport", "3"),
    int_capability("MdpropMdxDescFlags", "7"),
    int_capability("MdpropMdxFormulas", "63"),
    int_capability("MdpropMdxJoinCubes", "1"),
    int_capability("MdpropMdxMemberFunctions", "15"),
    int_capability("MdpropMdxNonMeasureExpressions", "0"),
    int_capability("MdpropMdxNumericFunctions", "2047"),
    int_capability("MdpropMdxObjQualification", "496"),
    int_capability("MdpropMdxOuterReference", "0"),
    int_capability("MdpropMdxRangeRowset", "4"),
    int_capability("MdpropMdxSetFunctions", "524287"),
    int_capability("MdpropMdxSlicer", "2"),
    int_capability("MdpropMdxStringCompop", "15"),
    int_capability("DbpropMsmdMDXCompatibility", "0"),
    int_capability("DbpropMsmdMDXUniqueNameStyle", "0"),
    Property {
        name: "MdxMissingMemberMode",
        description: "MdxMissingMemberMode",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("Default"),
    },
    Property {
        name: "MdpropMdxQueryByProperty",
        description: "MdpropMdxQueryByProperty",
        prop_type: "boolean",
        access_type: "Read",
        is_required: false,
        value: Some("true"),
    },
    Property {
        name: "StateSupport",
        description: "StateSupport",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Sessions"),
    },
];

const PROPERTY_ROW_FIELDS: &str = r#"                <xsd:element sql:field="PropertyName" name="PropertyName" type="xsd:string"/>
                <xsd:element sql:field="PropertyDescription" name="PropertyDescription" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyType" name="PropertyType" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyAccessType" name="PropertyAccessType" type="xsd:string"/>
                <xsd:element sql:field="IsRequired" name="IsRequired" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="Value" name="Value" type="xsd:string" minOccurs="0"/>"#;

fn format_row(p: &Property) -> String {
    let val = if p.name == "Catalog" {
        proxy_project::project().config.catalog.clone()
    } else {
        p.value.unwrap_or("").to_string()
    };
    format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{desc}</PropertyDescription>
            <PropertyType>{ptype}</PropertyType>
            <PropertyAccessType>{access}</PropertyAccessType>
            <IsRequired>{req}</IsRequired>
            <Value>{val}</Value>
          </row>"#,
        name = p.name,
        desc = p.description,
        ptype = p.prop_type,
        access = p.access_type,
        req = p.is_required,
        val = val,
    )
}

pub fn get_properties_response(filter: &[String]) -> String {
    let filtered: Vec<String> = PROPERTIES
        .iter()
        .filter(|p| filter.is_empty() || filter.iter().any(|f| f == p.name))
        .map(format_row)
        .collect();

    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &filtered.join("\n"))
}

pub fn get_single_property_response(name: &str, value: &str) -> String {
    let row = format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{name}</PropertyDescription>
            <PropertyType>string</PropertyType>
            <PropertyAccessType>ReadWrite</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>{value}</Value>
          </row>"#,
    );
    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &row)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Plan 046: named sets are not implemented, so the capability must not be
    // advertised (Excel otherwise offers "Manage Sets" and sends `WITH SET`).
    #[test]
    fn named_sets_are_advertised_as_unsupported() {
        let resp = get_properties_response(&["MdpropMdxNamedSets".to_string()]);
        assert!(
            resp.contains("<PropertyName>MdpropMdxNamedSets</PropertyName>"),
            "{resp}"
        );
        assert!(resp.contains("<Value>0</Value>"), "{resp}");
    }
}
