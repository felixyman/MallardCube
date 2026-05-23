/// nom-based parser for the Excel MDX subset.
///
/// Parses member references, WHERE clauses, property clauses,
/// and axis expressions from Excel MDX probe/query strings.

use nom::{
    IResult,
    bytes::complete::{tag, take_while},
    character::complete::{char, multispace0},
    sequence::delimited,
    branch::alt,
    multi::separated_list0,
    combinator::{value, map},
};

// ---- whitespace ----

fn ws(input: &str) -> IResult<&str, &str> {
    multispace0(input)
}

fn sp(input: &str) -> IResult<&str, &str> {
    take_while(|c: char| c == ' ' || c == '\t')(input)
}

// ---- identifiers ----

#[derive(Debug, Clone, PartialEq)]
pub enum DimKey {
    Produktkategori,
    Region,
    Measures,
}

fn dim_name(input: &str) -> IResult<&str, DimKey> {
    alt((
        value(DimKey::Produktkategori, tag("Produktkategori")),
        value(DimKey::Region, tag("Region")),
        value(DimKey::Measures, tag("Measures")),
    ))(input)
}

fn bracket<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(char('['), inner, char(']'))
}

fn bracket_str(input: &str) -> IResult<&str, &str> {
    let (input, _) = char('[')(input)?;
    let (input, inner) = take_while(|c: char| c != ']')(input)?;
    let (input, _) = char(']')(input)?;
    Ok((input, inner))
}

fn dim_hierarchy(input: &str) -> IResult<&str, (DimKey, &str)> {
    let (input, dim) = bracket(dim_name)(input)?;
    let (input, _) = char('.')(input)?;
    let (input, hname) = bracket_str(input)?;
    Ok((input, (dim, hname)))
}

// ---- member references ----

#[derive(Debug, Clone, PartialEq)]
pub enum MemberRef {
    All(DimKey),
    Leaf { dim: DimKey, key: String },
    Measure(String),
}

fn member_all(input: &str) -> IResult<&str, MemberRef> {
    let (input, (dim, _hier)) = dim_hierarchy(input)?;
    let (input, _) = alt((tag(".[All]"), tag(".[(All)]")))(input)?;
    Ok((input, MemberRef::All(dim)))
}

fn member_leaf(input: &str) -> IResult<&str, MemberRef> {
    let (input, (dim, _hier)) = dim_hierarchy(input)?;
    let (input, _) = tag(".&")(input)?;
    let (input, key) = bracket_str(input)?;
    Ok((input, MemberRef::Leaf { dim, key: key.to_string() }))
}

fn measure_member(input: &str) -> IResult<&str, MemberRef> {
    let (input, _) = bracket(dim_name)(input)?;
    let (input, _) = char('.')(input)?;
    let (input, name) = bracket_str(input)?;
    Ok((input, MemberRef::Measure(name.to_string())))
}

fn member_ref(input: &str) -> IResult<&str, MemberRef> {
    alt((member_all, member_leaf, measure_member))(input)
}

// ---- property parsing ----

fn dim_prop_name(input: &str) -> IResult<&str, &str> {
    alt((
        tag("PARENT_UNIQUE_NAME"),
        tag("HIERARCHY_UNIQUE_NAME"),
        tag("MEMBER_NAME"),
        tag("MEMBER_KEY"),
        tag("MEMBER_TYPE"),
        tag("MEMBER_VALUE"),
        tag("PARENT_LEVEL"),
        tag("PARENT_COUNT"),
        tag("CHILDREN_CARDINALITY"),
        tag("MEMBER_CAPTION"),
        tag("MEMBER_UNIQUE_NAME"),
        tag("LEVEL_NUMBER"),
        tag("LEVEL_UNIQUE_NAME"),
    ))(input)
}

fn qualified_dim_prop(input: &str) -> IResult<&str, &str> {
    let (input, _dim) = bracket(dim_name)(input)?;
    let (input, _) = char('.')(input)?;
    let (input, _hier) = bracket_str(input)?;
    let (input, prop) = dim_prop_name(input)?;
    Ok((input, prop))
}

fn dim_property_token(input: &str) -> IResult<&str, String> {
    alt((
        map(qualified_dim_prop, |s| s.to_string()),
        map(dim_prop_name, |s| s.to_string()),
    ))(input)
}

fn cell_property_token(input: &str) -> IResult<&str, String> {
    let (input, tok) = take_while(|c: char| c != ',')(input)?;
    Ok((input, tok.trim().to_uppercase()))
}

// ---- WHERE clause ----

fn where_clause(input: &str) -> IResult<&str, Vec<MemberRef>> {
    let (input, _) = tag("WHERE")(input)?;
    let (input, _) = sp(input)?;
    let (input, _) = char('(')(input)?;
    let (input, _) = ws(input)?;
    let (input, members) = separated_list0(
        delimited(ws, char(','), ws),
        member_ref,
    )(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char(')')(input)?;
    Ok((input, members))
}

// ---- subquery filter parsing ----

fn subquery_body(input: &str) -> IResult<&str, Vec<MemberRef>> {
    let (input, _) = tag("SELECT ")(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = tag("({")(input)?;
    let (input, _) = ws(input)?;
    let (input, members) = separated_list0(
        delimited(ws, char(','), ws),
        member_ref,
    )(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = tag("})")(input)?;
    Ok((input, members))
}

fn find_all_subquery_members(input: &str) -> Vec<Vec<MemberRef>> {
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = input[search_from..].find("SELECT ({") {
        let sub = &input[search_from + pos..];
        if let Ok((_, members)) = subquery_body(sub) {
            results.push(members);
        }
        search_from += pos + "SELECT (".len();
    }
    results
}

// ---- axis detection ----

fn detect_axis_dimension(input: &str) -> DimKey {
    if input.contains("[Region]") {
        DimKey::Region
    } else if input.contains("[Produktkategori]") {
        DimKey::Produktkategori
    } else {
        DimKey::Measures
    }
}

fn has_crossjoin(input: &str) -> bool {
    input.contains("CrossJoin(")
}

fn has_drilldown(input: &str) -> bool {
    input.contains("DrilldownLevel")
}

fn has_dot_members(input: &str) -> bool {
    input.contains(".Members")
}

fn has_dot_children(input: &str) -> bool {
    input.contains(".Children")
}

fn has_with_member_cchildren(input: &str) -> bool {
    input.contains("WITH MEMBER [Measures].cChildren")
}

// ---- property clause extraction ----

pub fn parse_dimension_properties(input: &str) -> Vec<String> {
    let up = input.to_uppercase();
    let Some(pos) = up.find("DIMENSION PROPERTIES ") else { return vec![] };
    let after = &input[pos + "DIMENSION PROPERTIES ".len()..];
    let end = after.find(" ON COLUMNS")
        .or_else(|| after.find(" ON ROWS"))
        .or_else(|| after.find(" FROM "))
        .or_else(|| after.find(" CELL PROPERTIES"))
        .unwrap_or(after.len());
    let raw = after[..end].trim();

    let known = &[
        "PARENT_UNIQUE_NAME","HIERARCHY_UNIQUE_NAME","MEMBER_NAME","MEMBER_KEY",
        "MEMBER_TYPE","MEMBER_VALUE","PARENT_LEVEL","PARENT_COUNT","CHILDREN_CARDINALITY",
    ];
    let mut props = Vec::new();
    for token in raw.split(',') {
        let tu = token.trim().to_uppercase();
        for prop in known {
            if tu.ends_with(prop) && !props.iter().any(|p| p == prop) {
                props.push(prop.to_string());
                break;
            }
        }
    }
    props
}

pub fn parse_cell_properties(input: &str) -> Vec<String> {
    let up = input.to_uppercase();
    let Some(pos) = up.find("CELL PROPERTIES ") else { return vec![] };
    let after = &input[pos + "CELL PROPERTIES ".len()..];
    after.split(',')
        .map(|t| t.trim().to_uppercase())
        .filter(|t| !t.is_empty() && !t.contains(" "))
        .collect()
}

// ---- WHERE clause extraction ----

pub fn find_where_clause(input: &str) -> Option<Vec<MemberRef>> {
    let start = input.find("WHERE")?;
    let sub = &input[start..];
    where_clause(sub).ok().map(|(_, m)| m)
}

// ---- query-shape detection ----

#[derive(Debug, Clone, PartialEq)]
pub enum CChildrenTarget {
    None,
    All,
    Measures,
    ProductLeaf(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CalculatedMembersPat {
    None,
    MeasureChildrenEmpty,
    LeafChildrenEmpty,
    AllLevelMembers,
    LeafLevelMembers,
}

fn detect_cchildren_target(input: &str) -> CChildrenTarget {
    let Some(start) = input.find("FilteredMembers As '") else {
        return CChildrenTarget::None;
    };
    let after_open = &input[start + "FilteredMembers As '".len()..];
    let Some(end) = after_open.find('\'') else {
        return CChildrenTarget::None;
    };
    let set = &after_open[..end];

    if set.contains("[Measures]") && !set.contains("[Produktkategori]") && !set.contains("[Region]") {
        return CChildrenTarget::Measures;
    }

    if (set.contains("[Produktkategori]") || set.contains("[Region]"))
        && (set.contains("&[") || set.contains("&amp;["))
    {
        if let Some(amp) = set.find("&[") {
            let begin = amp + 2;
            if let Some(closing) = set[begin..].find(']') {
                return CChildrenTarget::ProductLeaf(set[begin..begin + closing].to_string());
            }
        }
        if let Some(amp) = set.find("&amp;[") {
            let begin = amp + 5;
            if let Some(closing) = set[begin..].find(']') {
                return CChildrenTarget::ProductLeaf(set[begin..begin + closing].to_string());
            }
        }
    }

    CChildrenTarget::All
}

fn detect_calculated_members_pat(input: &str) -> CalculatedMembersPat {
    let Some(pos) = input.to_uppercase().find("ADDCALCULATEDMEMBERS({") else {
        return CalculatedMembersPat::None;
    };
    let rest = &input[pos..];

    if rest.contains("[Measures]") && rest.contains(".Children}") {
        return CalculatedMembersPat::MeasureChildrenEmpty;
    }

    if (rest.contains(".&[") || rest.contains(".&amp;[")) && rest.contains(".Children}") {
        return CalculatedMembersPat::LeafChildrenEmpty;
    }

    if rest.contains("[(All)]") && (rest.contains(".Members}") || rest.contains(".MEMBERS}")) {
        return CalculatedMembersPat::AllLevelMembers;
    }

    if rest.contains("[All]") && (rest.contains(".Children}") || rest.contains(".CHILDREN}")) {
        return CalculatedMembersPat::LeafLevelMembers;
    }

    if rest.contains(".Members}") || rest.contains(".MEMBERS}") {
        return CalculatedMembersPat::LeafLevelMembers;
    }

    CalculatedMembersPat::None
}

fn has_drilldown_member(input: &str) -> bool {
    input.contains("DrilldownMember(")
}

fn has_measures_in_where_or_cols(input: &str) -> bool {
    input.to_uppercase().contains("[MEASURES]")
}

fn is_slicer_all_measure(input: &str) -> bool {
    input.contains("WHERE ([Produktkategori].[Produktkategori].[All],[Measures].[Total Försäljning])")
        || input.contains("WHERE ([Region].[Region].[All],[Measures].[Total Försäljning])")
}

// ---- complete mdx parse ----

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMdx {
    pub dim_props: Vec<String>,
    pub cell_props: Vec<String>,
    pub has_rows: bool,
    pub has_cols: bool,
    pub has_crossjoin: bool,
    pub has_drilldown: bool,
    pub has_dot_members: bool,
    pub has_dot_children: bool,
    pub has_with_member_cchildren: bool,
    pub has_where_all_measure: bool,
    pub has_drilldown_member: bool,
    pub has_measures: bool,
    pub where_members: Vec<MemberRef>,
    pub subquery_members: Vec<MemberRef>,
    pub main_dim: DimKey,
    pub cchildren_target: CChildrenTarget,
    pub calculated_members_pat: CalculatedMembersPat,
}

pub fn parse_mdx(input: &str) -> ParsedMdx {
    let up = input.to_uppercase();
    let before_from = input.find("FROM [Model]")
        .or_else(|| input.find("FROM [model]"))
        .map(|i| &input[..i]).unwrap_or(input);

    let where_members = find_where_clause(input).unwrap_or_default();

    let all_subquery = find_all_subquery_members(input);
    let subquery_members: Vec<MemberRef> = all_subquery.into_iter().flatten().collect();

    ParsedMdx {
        dim_props: parse_dimension_properties(input),
        cell_props: parse_cell_properties(input),
        has_rows: up.contains("ON ROWS"),
        has_cols: up.contains("ON COLUMNS"),
        has_crossjoin: has_crossjoin(input),
        has_drilldown: has_drilldown(input),
        has_dot_members: has_dot_members(input),
        has_dot_children: has_dot_children(input),
        has_with_member_cchildren: has_with_member_cchildren(input),
        has_where_all_measure: is_slicer_all_measure(input),
        has_drilldown_member: has_drilldown_member(input),
        has_measures: has_measures_in_where_or_cols(input),
        where_members,
        subquery_members,
        main_dim: detect_axis_dimension(before_from),
        cchildren_target: detect_cchildren_target(input),
        calculated_members_pat: detect_calculated_members_pat(input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_member_all() {
        let (rest, m) = member_ref("[Produktkategori].[Produktkategori].[All]").unwrap();
        assert_eq!(m, MemberRef::All(DimKey::Produktkategori));
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_member_leaf() {
        let (rest, m) = member_ref("[Produktkategori].[Produktkategori].&[Kategori A]").unwrap();
        assert_eq!(m, MemberRef::Leaf { dim: DimKey::Produktkategori, key: "Kategori A".into() });
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_member_region() {
        let (rest, m) = member_ref("[Region].[Region].&[North]").unwrap();
        assert_eq!(m, MemberRef::Leaf { dim: DimKey::Region, key: "North".into() });
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_where_multiple() {
        let input = "WHERE ([Region].[Region].[All],[Measures].[Total Försäljning])";
        let (rest, members) = where_clause(input).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0], MemberRef::All(DimKey::Region));
        assert_eq!(members[1], MemberRef::Measure("Total Försäljning".into()));
    }

    #[test]
    fn parse_where_leaf() {
        let input = "WHERE ([Produktkategori].[Produktkategori].&[Kategori B],[Measures].[Total Försäljning])";
        let (rest, members) = where_clause(input).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0], MemberRef::Leaf { dim: DimKey::Produktkategori, key: "Kategori B".into() });
    }

    #[test]
    fn parse_subquery() {
        let input = "SELECT ({[Produktkategori].[Produktkategori].&[Kategori A],[Produktkategori].[Produktkategori].&[Kategori C]}) ON COLUMNS FROM [Model]";
        let (rest, m) = subquery_body("SELECT ({[Produktkategori].[Produktkategori].&[Kategori A],[Produktkategori].[Produktkategori].&[Kategori C]})").unwrap();
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn parse_dim_props() {
        let input = "SELECT ... DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,MEMBER_TYPE ON COLUMNS FROM [Model]";
        let props = parse_dimension_properties(input);
        assert!(props.contains(&"PARENT_UNIQUE_NAME".to_string()));
        assert!(props.contains(&"HIERARCHY_UNIQUE_NAME".to_string()));
        assert!(props.contains(&"MEMBER_TYPE".to_string()));
    }

    #[test]
    fn parse_cell_props() {
        let input = "SELECT ... CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
        let props = parse_cell_properties(input);
        assert_eq!(props, vec!["VALUE", "FORMAT_STRING", "BACK_COLOR", "FORE_COLOR"]);
    }
}
