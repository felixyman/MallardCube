/// Execute and cellset response layer.
///
/// Public API for statement execution — the entry point that `dispatch.rs`
/// and `main.rs` call.  Cellset rendering logic lives in `render.rs`;
/// Malloy runtime machinery lives in `runtime.rs`.
///
/// Legacy MDX/DAX flat-rowset helpers also live here as transitional code.

use crate::response::wrap_in_soap_envelope;
use crate::backend::{Backend, QueryBackend};
use crate::engine::plan::{QueryResult, execute_plan, execute_plan_with_sql, execute_plan_with_backend, plan_from_semantic};
use crate::engine::model::SemanticModel;
use crate::engine::normalize::plan_key;
use crate::engine::timing::{Timings, RuntimePath};
use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind};
use crate::execute::render::dispatch;

// Re-export Malloy runtime machinery at the same path callers expect.
pub use crate::execute::runtime::{
    USE_MALLOY_RUNTIME,
    enable_malloy_runtime,
    disable_malloy_runtime,
    warm_malloy_worker,
    get_execute_cellset_response_timed,
    get_execute_cellset_response_timed_malloy,
};

// ---- public API consumed by execute.rs dispatch ----

pub fn execute_semantic_query(query: &SemanticQuery) -> String {
    let plan = plan_from_semantic(query);
    let model = &crate::proxy_project::project().model;
    let result = execute_plan(&plan, model);
    dispatch(query, &result)
}

pub fn execute_semantic_query_with_backend<B: QueryBackend>(
    query: &SemanticQuery,
    backend: &B,
    model: &SemanticModel,
) -> String {
    let plan = plan_from_semantic(query);
    let result = execute_plan_with_backend(&plan, model, backend);
    dispatch(query, &result)
}

pub fn get_execute_cellset_response(mdx: &str) -> String {
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    execute_semantic_query(&query)
}

pub fn get_execute_cellset_response_with_backend<B: QueryBackend>(
    mdx: &str,
    backend: &B,
    model: &SemanticModel,
) -> String {
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    execute_semantic_query_with_backend(&query, backend, model)
}

// ---- legacy flat-rowset helpers (transitional) ----

pub fn get_execute_mdx_response(mdx: &str) -> String {
    let has_measures = mdx.contains("Measures") || mdx.contains("measures");
    let measure_name = "Total_Forsaljning";
    let measure_value = if has_measures { Backend::get().total_sales() } else { 0.0 };

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{name}" name="{name}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{name}>{val}</{name}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        name = measure_name,
        val = measure_value,
    );
    wrap_in_soap_envelope(&inner)
}

pub fn get_execute_dax_response(_dax: &str) -> String {
    let total = Backend::get().total_sales();
    let col_xml_name = "Faktatabell_x005B_Total_x0020_Försäljning_x0020__x0028_SEK_x0029__x005D_";
    let col_sql_field = "[Faktatabell].[Total Försäljning (SEK)]";

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{sqlf}" name="{xname}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{xname}>{val}</{xname}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        sqlf = col_sql_field,
        xname = col_xml_name,
        val = total,
    );
    wrap_in_soap_envelope(&inner)
}
