/// Cellset response builders.
///
/// Converts a `SemanticQuery` (from `mdx_semantic`) into a full
/// mddataset XML response, backed by the current `Backend`.
///
/// Also contains the flat-rowset fallback responses for MDX and DAX.
///
/// Member/cell/axis/slicer helpers live in `axis_members`.

use crate::response::wrap_in_soap_envelope;
use crate::backend::{Backend, QueryBackend};
use crate::engine::plan::{QueryResult, execute_plan, execute_plan_with_backend, plan_from_semantic};
use crate::engine::model::{default_model, SemanticModel};
use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind};
use crate::axis_members::{
    render_response, full_slicer_axis, measures_axis,
    single_member_axis, member_list_axis, empty_member_list_axis,
    row_dim, leaf_member_for, all_member_for, hierarchy_for, leaf_members_from,
    measurement_cell, count_cell, measures_hierarchy, measures_total_member,
    cchildren_member,
};

// ---- cellset response builders ----

fn ordered_pair(
    dims: &[String],
    d0: &str,
    m0: crate::cellset::MemberConfig,
    d1: &str,
    m1: crate::cellset::MemberConfig,
) -> crate::cellset::TupleConfig {
    let first = dims.first().map(|s| s.as_str()).unwrap_or(d0);
    if first == d1 {
        crate::cellset::TupleConfig { members: vec![m1, m0] }
    } else {
        crate::cellset::TupleConfig { members: vec![m0, m1] }
    }
}

/// Map a 2D SQL row (first, second, value) to (kat_value, region_value)
/// based on the visible axis dimension order.
fn map_pair_values<'a>(dims: &[String], first: &'a str, second: &'a str) -> (&'a str, &'a str) {
    let dim0 = dims.first().map(|s| s.as_str()).unwrap_or("Produktkategori");
    if dim0 == "Region" {
        (second, first)
    } else {
        (first, second)
    }
}

fn build_slicer_only(query: &SemanticQuery, result: &QueryResult) -> String {
    let total = match result {
        QueryResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![full_slicer_axis(query)],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_drilldown(query: &SemanticQuery, result: &QueryResult) -> String {
    let dims = &query.axis_dimensions;
    if dims.len() >= 2 {
        return build_drilldown_multi(query, result);
    }
    let dim = dims.first().map(|s| s.as_str()).unwrap_or("Produktkategori");
    let data = match result {
        QueryResult::Grouped(data) => data,
        _ => unreachable!(),
    };
    let members = leaf_members_from(dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
    );

    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell(i as u32, *value));
    }

    render_response(
        vec![
            member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), members),
            full_slicer_axis(query),
        ],
        cells,
        &query.cell_props,
    )
}

fn build_drilldown_multi(query: &SemanticQuery, result: &QueryResult) -> String {
    let dims = &query.axis_dimensions;
    let all_data = match result {
        QueryResult::Pairs(pairs) => pairs,
        _ => unreachable!(),
    };
    let has_exclusions = !query.excluded_members.is_empty();

    let mut hierarchies: Vec<crate::cellset::HierarchyConfig> = Vec::new();
    for dim in dims {
        hierarchies.push(hierarchy_for(dim, &query.dim_props));
    }

    let mut tuples: Vec<crate::cellset::TupleConfig> = Vec::new();
    let mut cells = Vec::new();
    let mut ordinal = 0u32;
    for (first, second, value) in all_data {
        let (kat, region) = map_pair_values(dims, first, second);
        if has_exclusions && query.excluded_members.iter().any(|e| e.key == kat) {
            continue;
        }
        let kat_member = leaf_member_for("Produktkategori", kat, &query.dim_props);
        let region_member = leaf_member_for("Region", region, &query.dim_props);
        tuples.push(ordered_pair(
            dims,
            "Produktkategori", kat_member,
            "Region", region_member,
        ));
        cells.push(measurement_cell(ordinal, *value));
        ordinal += 1;
    }

    let axis = crate::cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies,
        tuples,
    };

    render_response(
        vec![axis, full_slicer_axis(query)],
        cells,
        &query.cell_props,
    )
}

fn build_drilldown_member(query: &SemanticQuery, result: &QueryResult) -> String {
    let dims = &query.axis_dimensions;
    let all_data = match result {
        QueryResult::Pairs(pairs) => pairs,
        _ => unreachable!(),
    };
    let collapse_hier = query.drilldown_member_hierarchy.as_deref().unwrap_or("Region");

    let mut hierarchies: Vec<crate::cellset::HierarchyConfig> = Vec::new();
    for dim in dims {
        hierarchies.push(hierarchy_for(dim, &query.dim_props));
    }

    let mut tuples: Vec<crate::cellset::TupleConfig> = Vec::new();
    let mut cells = Vec::new();
    let mut ordinal = 0u32;

    let excluded_kats: std::collections::HashSet<&str> = query.excluded_members.iter()
        .filter(|e| e.dimension == "Produktkategori")
        .map(|e| e.key.as_str())
        .collect();
    let excluded_regions: std::collections::HashSet<&str> = query.excluded_members.iter()
        .filter(|e| e.dimension == "Region")
        .map(|e| e.key.as_str())
        .collect();
    let mut seen_kats: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut seen_regions: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for (first, second, value) in all_data {
        let (kat, region) = map_pair_values(dims, first, second);
        let is_kat_excluded = excluded_kats.contains(kat);
        let is_region_excluded = excluded_regions.contains(region);

        // Region collapse: excluded by Produktkategori member
        if collapse_hier == "Region" && is_kat_excluded {
            if !seen_kats.contains(kat) {
                let total = Backend::get().total_sales_for(kat);
                let kat_leaf = leaf_member_for("Produktkategori", kat, &query.dim_props);
                let region_all = all_member_for("Region", &query.dim_props);
                tuples.push(ordered_pair(
                    dims,
                    "Produktkategori", kat_leaf,
                    "Region", region_all,
                ));
                cells.push(measurement_cell(ordinal, total));
                ordinal += 1;
                seen_kats.insert(kat);
            }
            continue;
        }

        // Produktkategori collapse: excluded by Produktkategori member
        if collapse_hier == "Produktkategori" && is_kat_excluded {
            if !seen_regions.contains(region) {
                let kat_all = all_member_for("Produktkategori", &query.dim_props);
                let region_leaf = leaf_member_for("Region", region, &query.dim_props);
                tuples.push(ordered_pair(
                    dims,
                    "Produktkategori", kat_all,
                    "Region", region_leaf,
                ));
                cells.push(measurement_cell(ordinal, *value));
                ordinal += 1;
                seen_regions.insert(region);
            }
            continue;
        }

        // Produktkategori collapse: excluded by Region member
        if collapse_hier == "Produktkategori" && is_region_excluded {
            if !seen_regions.contains(region) {
                let total = Backend::get().total_sales_for_region(region);
                let region_leaf = leaf_member_for("Region", region, &query.dim_props);
                let kat_all = all_member_for("Produktkategori", &query.dim_props);
                tuples.push(ordered_pair(
                    dims,
                    "Region", region_leaf,
                    "Produktkategori", kat_all,
                ));
                cells.push(measurement_cell(ordinal, total));
                ordinal += 1;
                seen_regions.insert(region);
            }
            continue;
        }

        let kat_member = leaf_member_for("Produktkategori", kat, &query.dim_props);
        let region_member = leaf_member_for("Region", region, &query.dim_props);
        tuples.push(ordered_pair(
            dims,
            "Produktkategori", kat_member,
            "Region", region_member,
        ));
        cells.push(measurement_cell(ordinal, *value));
        ordinal += 1;
    }

    let axis = crate::cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies,
        tuples,
    };

    render_response(
        vec![axis, full_slicer_axis(query)],
        cells,
        &query.cell_props,
    )
}

fn build_measure_by_category(query: &SemanticQuery, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let data = match result {
        QueryResult::Grouped(data) => data,
        _ => unreachable!(),
    };
    let axis1_members = leaf_members_from(dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
    );
    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell(i as u32, *value));
    }

    render_response(
        vec![
            measures_axis(),
            member_list_axis("Axis1", hierarchy_for(dim, &query.dim_props), axis1_members),
            full_slicer_axis(query),
        ],
        cells,
        &query.cell_props,
    )
}

fn build_slicer_all_and_measure(query: &SemanticQuery, result: &QueryResult) -> String {
    let total = match result {
        QueryResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![full_slicer_axis(query)],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_all_level_members(query: &SemanticQuery, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let total = match result {
        QueryResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![
            single_member_axis("Axis0", hierarchy_for(dim, &query.dim_props), all_member_for(dim, &query.dim_props)),
            full_slicer_axis(query),
        ],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_leaf_level_members(query: &SemanticQuery, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let data = match result {
        QueryResult::Grouped(data) => data,
        _ => unreachable!(),
    };
    let members = leaf_members_from(dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
    );
    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell(i as u32, *value));
    }

    render_response(
        vec![
            member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), members),
            full_slicer_axis(query),
        ],
        cells,
        &query.cell_props,
    )
}

fn build_leaf_children_empty(query: &SemanticQuery, _result: &QueryResult) -> String {
    let dim = row_dim(query);
    render_response(
        vec![
            empty_member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props)),
            full_slicer_axis(query),
        ],
        vec![],
        &query.cell_props,
    )
}

fn build_measure_children_empty(query: &SemanticQuery, _result: &QueryResult) -> String {
    render_response(
        vec![
            empty_member_list_axis("Axis0", measures_hierarchy()),
            full_slicer_axis(query),
        ],
        vec![],
        &query.cell_props,
    )
}

fn build_cchildren_for_all(query: &SemanticQuery, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let count = match result {
        QueryResult::Count(c) => *c,
        _ => unreachable!(),
    };
    render_response(
        vec![
            single_member_axis("Axis0", hierarchy_for(dim, &query.dim_props), all_member_for(dim, &query.dim_props)),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            full_slicer_axis(query),
        ],
        vec![count_cell(0, count)],
        &query.cell_props,
    )
}

fn build_cchildren_for_leaf_product(query: &SemanticQuery, name: &str, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let leaf = leaf_member_for(dim, name, &query.dim_props);
    let all = all_member_for(dim, &query.dim_props);
    let real_count = match result {
        QueryResult::Count(c) => *c,
        _ => unreachable!(),
    };
    render_response(
        vec![
            member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), vec![all, leaf]),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            full_slicer_axis(query),
        ],
        vec![count_cell(0, real_count), count_cell(1, 0)],
        &query.cell_props,
    )
}

fn build_cchildren_for_measures(query: &SemanticQuery, _result: &QueryResult) -> String {
    render_response(
        vec![
            single_member_axis("Axis0", measures_hierarchy(), measures_total_member()),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            full_slicer_axis(query),
        ],
        vec![count_cell(0, 0)],
        &query.cell_props,
    )
}

// ---- public API consumed by execute.rs dispatch ----

pub fn execute_semantic_query(query: &SemanticQuery) -> String {
    let plan = plan_from_semantic(query);
    let result = execute_plan(&plan, &default_model());
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

fn dispatch(query: &SemanticQuery, result: &QueryResult) -> String {
    match query.kind {
        SemanticQueryKind::ChildrenCountForAll => build_cchildren_for_all(query, &result),
        SemanticQueryKind::ChildrenCountLeafProduct => {
            let name = query.cchildren_leaf_name.as_deref().unwrap_or("");
            build_cchildren_for_leaf_product(query, name, &result)
        }
        SemanticQueryKind::ChildrenCountMeasures => build_cchildren_for_measures(query, &result),
        SemanticQueryKind::SlicerAllAndMeasure => build_slicer_all_and_measure(query, &result),
        SemanticQueryKind::MeasureChildrenEmpty => build_measure_children_empty(query, &result),
        SemanticQueryKind::LeafChildrenEmpty => build_leaf_children_empty(query, &result),
        SemanticQueryKind::AllLevelMembers => build_all_level_members(query, &result),
        SemanticQueryKind::LeafLevelMembers => build_leaf_level_members(query, &result),
        SemanticQueryKind::MeasureByCategory => build_measure_by_category(query, &result),
        SemanticQueryKind::DrilldownCategories => build_drilldown(query, &result),
        SemanticQueryKind::SlicerOnly => build_slicer_only(query, &result),
        SemanticQueryKind::DrilldownMemberProbe => build_drilldown_member(query, &result),
    }
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
