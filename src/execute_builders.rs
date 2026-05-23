/// Cellset response builders.
///
/// Converts a `SemanticQuery` (from `mdx_semantic`) into a full
/// mddataset XML response, backed by the current `Backend`.
///
/// Also contains the flat-rowset fallback responses for MDX and DAX.
///
/// Member/cell/axis/slicer helpers live in `axis_members`.

use crate::response::wrap_in_soap_envelope;
use crate::backend::Backend;
use crate::engine::plan::{PlanResult, execute_plan, plan_from_semantic};
use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind};
use crate::axis_members::{
    render_response, full_slicer_axis, measures_axis,
    single_member_axis, member_list_axis, empty_member_list_axis,
    row_dim, leaf_member_for, all_member_for, hierarchy_for, leaf_members_from,
    measurement_cell, count_cell, measures_hierarchy, measures_total_member,
    cchildren_member,
};

// ---- cellset response builders ----

fn build_slicer_only(query: &SemanticQuery, result: &PlanResult) -> String {
    let total = match result {
        PlanResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![full_slicer_axis(query)],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_drilldown(query: &SemanticQuery, result: &PlanResult) -> String {
    let dims = &query.axis_dimensions;
    if dims.len() >= 2 {
        return build_drilldown_multi(query, result);
    }
    let dim = dims.first().map(|s| s.as_str()).unwrap_or("Produktkategori");
    let data = match result {
        PlanResult::Grouped(data) => data,
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

fn build_drilldown_multi(query: &SemanticQuery, result: &PlanResult) -> String {
    let dims = &query.axis_dimensions;
    let all_data = match result {
        PlanResult::Paired(pairs) => pairs,
        PlanResult::PairedCollapsed { pairs, .. } => pairs,
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
    for (kat, region, value) in all_data {
        if has_exclusions && query.excluded_members.contains(kat) {
            continue;
        }
        let kat_member = leaf_member_for("Produktkategori", kat, &query.dim_props);
        let region_member = leaf_member_for("Region", region, &query.dim_props);
        tuples.push(crate::cellset::TupleConfig { members: vec![kat_member, region_member] });
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

fn build_drilldown_member(query: &SemanticQuery, result: &PlanResult) -> String {
    let dims = &query.axis_dimensions;
    let (all_data, total_per_excluded) = match result {
        PlanResult::PairedCollapsed { pairs, total_per_excluded } => (pairs, total_per_excluded),
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

    let excluded_kats: std::collections::HashSet<&str> =
        query.excluded_members.iter().map(|s| s.as_str()).collect();
    let total_map: std::collections::HashMap<&str, f64> =
        total_per_excluded.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let mut seen_kats: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut seen_regions: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for (kat, region, value) in all_data {
        let is_excluded = excluded_kats.contains(kat.as_str());

        if collapse_hier == "Region" && is_excluded {
            if !seen_kats.contains(kat.as_str()) {
                let total = total_map.get(kat.as_str()).copied().unwrap_or(0.0);
                tuples.push(crate::cellset::TupleConfig {
                    members: vec![
                        leaf_member_for("Produktkategori", kat, &query.dim_props),
                        all_member_for("Region", &query.dim_props),
                    ],
                });
                cells.push(measurement_cell(ordinal, total));
                ordinal += 1;
                seen_kats.insert(kat);
            }
            continue;
        }

        if collapse_hier == "Produktkategori" && is_excluded {
            if !seen_regions.contains(region.as_str()) {
                tuples.push(crate::cellset::TupleConfig {
                    members: vec![
                        all_member_for("Produktkategori", &query.dim_props),
                        leaf_member_for("Region", region, &query.dim_props),
                    ],
                });
                cells.push(measurement_cell(ordinal, *value));
                ordinal += 1;
                seen_regions.insert(region);
            }
            continue;
        }

        let kat_member = leaf_member_for("Produktkategori", kat, &query.dim_props);
        let region_member = leaf_member_for("Region", region, &query.dim_props);
        tuples.push(crate::cellset::TupleConfig { members: vec![kat_member, region_member] });
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

fn build_measure_by_category(query: &SemanticQuery, result: &PlanResult) -> String {
    let dim = row_dim(query);
    let data = match result {
        PlanResult::Grouped(data) => data,
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

fn build_slicer_all_and_measure(query: &SemanticQuery, result: &PlanResult) -> String {
    let total = match result {
        PlanResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![full_slicer_axis(query)],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_all_level_members(query: &SemanticQuery, result: &PlanResult) -> String {
    let dim = row_dim(query);
    let total = match result {
        PlanResult::Scalar(v) => *v,
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

fn build_leaf_level_members(query: &SemanticQuery, result: &PlanResult) -> String {
    let dim = row_dim(query);
    let data = match result {
        PlanResult::Grouped(data) => data,
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

fn build_leaf_children_empty(query: &SemanticQuery, _result: &PlanResult) -> String {
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

fn build_measure_children_empty(query: &SemanticQuery, _result: &PlanResult) -> String {
    render_response(
        vec![
            empty_member_list_axis("Axis0", measures_hierarchy()),
            full_slicer_axis(query),
        ],
        vec![],
        &query.cell_props,
    )
}

fn build_cchildren_for_all(query: &SemanticQuery, result: &PlanResult) -> String {
    let dim = row_dim(query);
    let count = match result {
        PlanResult::Count(c) => *c,
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

fn build_cchildren_for_leaf_product(query: &SemanticQuery, name: &str, result: &PlanResult) -> String {
    let dim = row_dim(query);
    let leaf = leaf_member_for(dim, name, &query.dim_props);
    let all = all_member_for(dim, &query.dim_props);
    let real_count = match result {
        PlanResult::Count(c) => *c,
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

fn build_cchildren_for_measures(query: &SemanticQuery, _result: &PlanResult) -> String {
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
    let result = execute_plan(&plan);
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
