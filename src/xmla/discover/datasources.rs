/// DISCOVER_DATASOURCES — returns the data source list for this server.
/// Required by Excel CUBE functions to validate the server has at least
/// one accessible data source.
use crate::response::discover_rowset_envelope;

const DATASOURCES_FIELDS: &str = r#"                <xsd:element sql:field="DataSourceName" name="DataSourceName" type="xsd:string"/>
                <xsd:element sql:field="DataSourceDescription" name="DataSourceDescription" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="URL" name="URL" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DataSourceInfo" name="DataSourceInfo" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="ProviderName" name="ProviderName" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="ProviderType" name="ProviderType" type="xsd:string"/>
                <xsd:element sql:field="AuthenticationMode" name="AuthenticationMode" type="xsd:string"/>"#;

pub fn get_datasources_response() -> String {
    let rows = r#"          <row>
            <DataSourceName>MallardCube</DataSourceName>
            <DataSourceDescription>MallardCube DuckDB embedded data source</DataSourceDescription>
            <ProviderName>DuckDB</ProviderName>
            <ProviderType>MDP</ProviderType>
            <AuthenticationMode>Unauthenticated</AuthenticationMode>
          </row>"#;

    discover_rowset_envelope("", DATASOURCES_FIELDS, rows)
}
