use crate::response::{discover_rowset_envelope, UUID_TYPE};

struct Property {
    name: &'static str,
    description: &'static str,
    prop_type: &'static str,
    access_type: &'static str,
    is_required: bool,
    value: Option<&'static str>,
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
        value: Some("2"),
    },
    Property {
        name: "DbpropMsmdOptimizeResponse",
        description: "DbpropMsmdOptimizeResponse",
        prop_type: "long",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("0"),
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
        value: Some("KTH_KEX_MALLOY_CUBE"),
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
        value: Some("1.0.0"),
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
        value: Some("15"),
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
];

const PROPERTY_ROW_FIELDS: &str = r#"                <xsd:element sql:field="PropertyName" name="PropertyName" type="xsd:string"/>
                <xsd:element sql:field="PropertyDescription" name="PropertyDescription" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyType" name="PropertyType" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyAccessType" name="PropertyAccessType" type="xsd:string"/>
                <xsd:element sql:field="IsRequired" name="IsRequired" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="Value" name="Value" type="xsd:string" minOccurs="0"/>"#;

fn format_row(p: &Property) -> String {
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
        val = p.value.unwrap_or(""),
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
