use crate::response::{discover_rowset_envelope, xml_escape};

pub struct ColumnDef {
    pub field: &'static str,
    pub name: &'static str,
    pub type_: &'static str,
    pub min_occurs: bool,
}

#[derive(Clone)]
pub struct Row {
    cols: Vec<(String, String)>,
}

impl Row {
    pub fn new(cols: Vec<(String, String)>) -> Self {
        Self { cols }
    }

    pub fn value(&self, field: &str) -> Option<&str> {
        self.cols
            .iter()
            .find(|(f, _)| f == field)
            .map(|(_, v)| v.as_str())
    }

    pub fn i32_val(&self, field: &str) -> Option<i32> {
        self.value(field).and_then(|v| v.parse().ok())
    }

    pub fn iter(&self) -> impl Iterator<Item = &(String, String)> {
        self.cols.iter()
    }
}

pub struct Rowset {
    pub columns: &'static [ColumnDef],
    pub rows: Vec<Row>,
    pub extra_schema: &'static str,
}

impl Rowset {
    pub fn to_xml(&self) -> String {
        let mut fields = String::new();
        for col in self.columns {
            let min = if col.min_occurs {
                r#" minOccurs="0""#
            } else {
                ""
            };
            fields.push_str(&format!(
                r#"                <xsd:element sql:field="{f}" name="{n}" type="{t}"{m}/>
"#,
                f = col.field,
                n = col.name,
                t = col.type_,
                m = min,
            ));
        }

        let mut rows_xml = String::new();
        for row in &self.rows {
            rows_xml.push_str("          <row>\n");
            for (field, val) in row.iter() {
                // Omit optional columns that have empty values.
                let is_optional_empty = self
                    .columns
                    .iter()
                    .any(|c| c.field == *field && c.min_occurs && val.is_empty());
                if is_optional_empty {
                    continue;
                }
                if let Some(col) = self.columns.iter().find(|c| c.field == *field) {
                    rows_xml.push_str(&format!(
                        "            <{n}>{v}</{n}>\n",
                        n = col.name,
                        v = xml_escape(val)
                    ));
                }
            }
            rows_xml.push_str("          </row>\n");
        }

        discover_rowset_envelope(self.extra_schema, &fields, &rows_xml)
    }
}
