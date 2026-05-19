use axum::{
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::net::SocketAddr;


#[derive(Debug, PartialEq)]
enum XmlaRequest {
    DiscoverProperties { property_name: Option<String> },
    DiscoverSchemaRowsets,
    DbSchemaCatalogs,
    MdschemaCubes,
    DbschemaTables,
    BeginSession,
    ExecuteEmpty,
    Unknown,
}
#[tokio::main]
async fn main() {
    let app = Router::new().route("/xmla", post(handle_xmla));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Rust-XMLA Proxy (v2 - Parsad) snurrar på http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_xmla(body: String) -> impl IntoResponse {
  if body.contains("<RequestType>") {
        let req_start = body.find("<RequestType>").unwrap() + 13;
        let req_end = body.find("</RequestType>").unwrap();
        println!("🔍 Rå RequestType från Excel: {}", &body[req_start..req_end]);
    }
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/xml; charset=utf-8".parse().unwrap());
    headers.insert(header::SERVER, "Rust-Malloy-Proxy/2.0".parse().unwrap());
    headers.insert(header::CONNECTION, "close".parse().unwrap());
    headers.insert(
        HeaderName::from_static("x-transport-caps-negotiation-flags"),
        "0,0,0,0,0".parse().unwrap(),
    );

    // Vi skickar in texten i vår parser som returnerar exakt vad Excel vill
    let request_type = parse_xmla(&body);
    println!("📥 Fick anrop, tolkade som: {:?}", request_type);

    if body.contains("<Execute") {
        println!("🔍 Rå Execute från Excel:\n{}", body);
    }
    // Match-satsen tvingar oss att hantera alla scenarier (inga fler if-satser!)
    let response_body = match request_type {
        XmlaRequest::BeginSession | XmlaRequest::ExecuteEmpty => get_empty_execute_response(),
        
        // HÄR KAN VI NU SE VAD EXCEL FRÅGAR EFTER!
        XmlaRequest::DiscoverProperties { property_name } => {
            println!("Excel frågar efter egenskap: {:?}", property_name);
            match property_name.as_deref() {
                Some("Catalog") => get_single_property_response("Catalog", "KTH_KEX_MALLOY_CUBE"),
                _ => get_properties_response(), 
            }
        },
        
        XmlaRequest::DiscoverSchemaRowsets => get_schemas_response(),
        XmlaRequest::DbSchemaCatalogs => get_catalogs_response(),
        XmlaRequest::MdschemaCubes => get_cubes_response(),     // Ny!
        XmlaRequest::DbschemaTables => get_tables_response(),   // Ny!
        XmlaRequest::Unknown => {
            println!("❌ Okänt anrop.");
            return (StatusCode::BAD_REQUEST, headers, "Okänt anrop".to_string());
        }
    };

    (StatusCode::OK, headers, response_body)
}

// 2. Vår Lexer/Parser som säkert plockar ut datan oavsett hur ful XML:en är
fn parse_xmla(xml: &str) -> XmlaRequest {
    let mut reader = Reader::from_str(xml);

    let mut in_request_type = false;
    let mut is_execute = false;
    let mut in_property_name = false;
    let mut is_begin_session = false;

    // Här sparar vi datan tills loopen är helt klar
    let mut parsed_request_type = String::new();
    let mut requested_property = None;

    // Vi "loopar" igenom XML-dokumentet nod för nod till slutet
    loop {
        match reader.read_event() {
            // Hittar en start-tagg (t.ex. <RequestType>, <PropertyName>, <BeginSession>)
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = true,
                    b"PropertyName" => in_property_name = true,
                    b"BeginSession" | b"BeginGetSessionToken" => is_begin_session = true,
                    b"Execute" => is_execute = true,
                    _ => (),
                }
            }
            // Läser text inuti taggarna
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                
                // Ignorera tomma strängar (radbrytningar etc)
                if !text.is_empty() {
                    if in_request_type {
                        parsed_request_type = text;
                    } else if in_property_name {
                        // Fångar "Catalog" när Excel frågar efter det
                        requested_property = Some(text);
                    }
                }
            }
            // När en tagg stängs
            Ok(Event::End(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = false,
                    b"PropertyName" => in_property_name = false,
                    _ => (),
                }
            }
            Ok(Event::Eof) => break, // Slut på filen, bryt loopen!
            Err(_) => break,
            _ => (),
        }
    }

    // NU, när vi har läst hela dokumentet, utvärderar vi vad vi hittade:
    match parsed_request_type.as_str() {
        "DISCOVER_PROPERTIES" => return XmlaRequest::DiscoverProperties { 
            property_name: requested_property 
        },
        "DISCOVER_SCHEMA_ROWSETS" => return XmlaRequest::DiscoverSchemaRowsets,
        "DBSCHEMA_CATALOGS" => return XmlaRequest::DbSchemaCatalogs,
        "MDSCHEMA_CUBES" => return XmlaRequest::MdschemaCubes,
        "DBSCHEMA_TABLES" => return XmlaRequest::DbschemaTables,
        _ => () // Fortsätt kolla om det var en Execute
    };

    // Hantera Execute-anropen (för sessioner och framtida MDX)
    if is_execute {
        if is_begin_session {
            return XmlaRequest::BeginSession;
        } else {
            // Excel skickar tomma Execute-anrop ibland bara för att hålla sessionen vid liv
            return XmlaRequest::ExecuteEmpty; 
        }
    }

    XmlaRequest::Unknown
}

// ==========================================
// DINA FÄRDIGA XML-SVAR (Orörda från förut)
// ==========================================

/// En hjälpfunktion som lägger in innehållet i ett komplett SOAP-kuvert
fn wrap_in_soap_envelope(inner_xml: &str) -> String {
    format!(
        r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="RUST-SESSION-456" />
  </soap:Header>
  <soap:Body>
{}
  </soap:Body>
</soap:Envelope>"#,
        inner_xml
    )
}

fn get_empty_execute_response() -> String {
    wrap_in_soap_envelope(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:empty"/>
      </return>
    </ExecuteResponse>"#
    )
}

fn get_schemas_response() -> String {
    wrap_in_soap_envelope(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence><xsd:element name="row" type="row" minOccurs="0" maxOccurs="unbounded"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:simpleType name="uuid">
              <xsd:restriction base="xsd:string">
                <xsd:pattern value="[0-9a-zA-Z]{8}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{12}"/>
              </xsd:restriction>
            </xsd:simpleType>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="SchemaName" name="SchemaName" type="xsd:string"/>
                <xsd:element sql:field="SchemaGuid" name="SchemaGuid" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="Restrictions" name="Restrictions" minOccurs="0" maxOccurs="unbounded">
                  <xsd:complexType>
                    <xsd:sequence>
                      <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                      <xsd:element sql:field="Type" name="Type" type="xsd:string" minOccurs="0"/>
                    </xsd:sequence>
                  </xsd:complexType>
                </xsd:element>
                <xsd:element sql:field="RestrictionsMask" name="RestrictionsMask" type="xsd:unsignedLong" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <SchemaName>DBSCHEMA_CATALOGS</SchemaName>
            <SchemaGuid>C8B52211-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_TABLES</SchemaName>
            <SchemaGuid>C8B52229-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>TABLE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_OLAP_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_CUBES</SchemaName>
            <SchemaGuid>C8B522D8-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>BASE_CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_DIMENSIONS</SchemaName>
            <SchemaGuid>C8B522D9-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASURES</SchemaName>
            <SchemaGuid>C8B522DA-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>255</RestrictionsMask>
          </row>
        </root>
      </return>
    </DiscoverResponse>"#
    )
}

fn get_catalogs_response() -> String {
    wrap_in_soap_envelope(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence><xsd:element name="row" type="row" minOccurs="0" maxOccurs="unbounded"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
          </row>
        </root>
      </return>
    </DiscoverResponse>"#
    )
}

fn get_cubes_response() -> String {
    wrap_in_soap_envelope(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:simpleType name="uuid">
              <xsd:restriction base="xsd:string">
                <xsd:pattern value="[0-9a-zA-Z]{8}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{12}"/>
              </xsd:restriction>
            </xsd:simpleType>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_TYPE" name="CUBE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_GUID" name="CUBE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="CREATED_ON" name="CREATED_ON" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="LAST_SCHEMA_UPDATE" name="LAST_SCHEMA_UPDATE" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="SCHEMA_UPDATED_BY" name="SCHEMA_UPDATED_BY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LAST_DATA_UPDATE" name="LAST_DATA_UPDATE" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="DATA_UPDATED_BY" name="DATA_UPDATED_BY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_DRILLTHROUGH_ENABLED" name="IS_DRILLTHROUGH_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_LINKABLE" name="IS_LINKABLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_WRITE_ENABLED" name="IS_WRITE_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_SQL_ENABLED" name="IS_SQL_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="CUBE_CAPTION" name="CUBE_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="BASE_CUBE_NAME" name="BASE_CUBE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_SOURCE" name="CUBE_SOURCE" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="PREFERRED_QUERY_PATTERNS" name="PREFERRED_QUERY_PATTERNS" type="xsd:unsignedShort" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <CUBE_TYPE>CUBE</CUBE_TYPE>
            <LAST_SCHEMA_UPDATE>2026-05-20T12:00:00.000000</LAST_SCHEMA_UPDATE>
            <LAST_DATA_UPDATE>2026-05-20T12:00:00.000000</LAST_DATA_UPDATE>
            <DESCRIPTION>Byggt med Rust och DuckDB!</DESCRIPTION>
            <IS_DRILLTHROUGH_ENABLED>true</IS_DRILLTHROUGH_ENABLED>
            <IS_LINKABLE>false</IS_LINKABLE>
            <IS_WRITE_ENABLED>false</IS_WRITE_ENABLED>
            <IS_SQL_ENABLED>false</IS_SQL_ENABLED>
            <CUBE_CAPTION>Malloy Analytics Cube</CUBE_CAPTION>
            <CUBE_SOURCE>1</CUBE_SOURCE>
            <PREFERRED_QUERY_PATTERNS>3</PREFERRED_QUERY_PATTERNS>
          </row>
        </root>
      </return>
    </DiscoverResponse>"#
    )
}

fn get_tables_response() -> String {
    wrap_in_soap_envelope(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence><xsd:element name="row" type="row" minOccurs="0" maxOccurs="unbounded"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:simpleType name="uuid">
              <xsd:restriction base="xsd:string">
                <xsd:pattern value="[0-9a-zA-Z]{8}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{12}"/>
              </xsd:restriction>
            </xsd:simpleType>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="TABLE_CATALOG" name="TABLE_CATALOG" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_SCHEMA" name="TABLE_SCHEMA" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_NAME" name="TABLE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_TYPE" name="TABLE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_GUID" name="TABLE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_PROPID" name="TABLE_PROPID" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="DATE_CREATED" name="DATE_CREATED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="DATE_MODIFIED" name="DATE_MODIFIED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="TABLE_OLAP_TYPE" name="TABLE_OLAP_TYPE" type="xsd:string" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_SCHEMA>Model</TABLE_SCHEMA> 
            <TABLE_NAME>Faktatabell</TABLE_NAME>
            <TABLE_TYPE>TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>CUBE_DIMENSION</TABLE_OLAP_TYPE>
          </row>
        </root>
      </return>
    </DiscoverResponse>"#
    )
}

fn get_properties_response() -> String {
    wrap_in_soap_envelope(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType>
                <xsd:sequence>
                  <xsd:element name="row" type="row" minOccurs="0" maxOccurs="unbounded" />
                </xsd:sequence>
              </xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element name="PropertyName" type="xsd:string" sql:field="PropertyName" />
                <xsd:element name="PropertyDescription" type="xsd:string" minOccurs="0" sql:field="PropertyDescription" />
                <xsd:element name="PropertyType" type="xsd:string" minOccurs="0" sql:field="PropertyType" />
                <xsd:element name="PropertyAccessType" type="xsd:string" minOccurs="0" sql:field="PropertyAccessType" />
                <xsd:element name="IsRequired" type="xsd:boolean" minOccurs="0" sql:field="IsRequired" />
                <xsd:element name="Value" type="xsd:string" minOccurs="0" sql:field="Value" />
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <PropertyName>ProviderName</PropertyName>
            <PropertyDescription>Namn</PropertyDescription>
            <PropertyType>DBTYPE_WSTR</PropertyType>
            <PropertyAccessType>Read</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>Min Riktiga Rust Proxy</Value>
          </row>
          <row>
            <PropertyName>DbpropMsmdSubqueries</PropertyName>
            <PropertyDescription>Subqueries</PropertyDescription>
            <PropertyType>DBTYPE_I4</PropertyType>
            <PropertyAccessType>Read</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>2</Value>
          </row>
          <row>
            <PropertyName>DbpropMsmdOptimizeResponse</PropertyName>
            <PropertyDescription>Optimize</PropertyDescription>
            <PropertyType>DBTYPE_I4</PropertyType>
            <PropertyAccessType>Read</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>7</Value>
          </row>
          <row>
            <PropertyName>MDXSupport</PropertyName>
            <PropertyDescription>MDX</PropertyDescription>
            <PropertyType>DBTYPE_WSTR</PropertyType>
            <PropertyAccessType>Read</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>Core</Value>
          </row>
        </root>
      </return>
    </DiscoverResponse>"#
    )
}

fn get_single_property_response(name: &str, value: &str) -> String {
    let inner = format!(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence><xsd:element name="row" type="row" minOccurs="0" maxOccurs="unbounded"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="PropertyName" name="PropertyName" type="xsd:string" />
                <xsd:element sql:field="PropertyDescription" name="PropertyDescription" type="xsd:string" minOccurs="0" />
                <xsd:element sql:field="PropertyType" name="PropertyType" type="xsd:string" minOccurs="0" />
                <xsd:element sql:field="PropertyAccessType" name="PropertyAccessType" type="xsd:string" minOccurs="0" />
                <xsd:element sql:field="IsRequired" name="IsRequired" type="xsd:boolean" minOccurs="0" />
                <xsd:element sql:field="Value" name="Value" type="xsd:string" minOccurs="0" />
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <PropertyName>{}</PropertyName>
            <PropertyDescription>{}</PropertyDescription>
            <PropertyType>string</PropertyType>
            <PropertyAccessType>ReadWrite</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>{}</Value>
          </row>
        </root>
      </return>
    </DiscoverResponse>"#,
        name, name, value
    );
    wrap_in_soap_envelope(&inner)
}