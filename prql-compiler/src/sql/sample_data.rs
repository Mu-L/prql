use anyhow::Result;
use itertools::Itertools;

use std::iter::zip;

use crate::ast::rq::RelationLiteral;

// TODO: Can we dynamically get the types, like in pandas? We need to put
// quotes around strings and not around numbers.
// https://stackoverflow.com/questions/64369887/how-do-i-read-csv-data-without-knowing-the-structure-at-compile-time
fn data_of_csv(csv: &str) -> Result<RelationLiteral> {
    let mut rdr = csv::Reader::from_reader(csv.as_bytes());

    Ok(RelationLiteral {
        columns: rdr
            .headers()?
            .into_iter()
            .map(|h| h.to_string())
            .collect::<Vec<_>>(),
        rows: rdr
            .records()
            .into_iter()
            // This is messy rust, but I can't get it to resolve the Errors
            // when it leads with `row_result?`. I'm sure it's possible...
            .map(|row_result| {
                row_result.map(|row| row.into_iter().map(|x| x.to_string()).collect())
            })
            .try_collect()?,
    })
}

use sqlparser::ast::{self as sql_ast, Select, SelectItem, SetExpr};

pub fn sql_of_sample_data(data: &RelationLiteral) -> sql_ast::Query {
    let mut selects = vec![];

    for row in &data.rows {
        // This seems *very* verbose. Maybe we put an issue into sqlparser-rs to
        // have something like a builder for these?
        let body = sql_ast::SetExpr::Select(Box::new(Select {
            distinct: false,
            top: None,
            from: vec![],
            projection: zip(data.columns.clone(), row)
                .map(|(col, value)| SelectItem::ExprWithAlias {
                    expr: sql_ast::Expr::Identifier(sql_ast::Ident {
                        value: value.into(),
                        quote_style: None,
                    }),
                    alias: sql_ast::Ident {
                        value: col,
                        quote_style: None,
                    },
                })
                .collect(),
            selection: None,
            group_by: vec![],
            having: None,
            lateral_views: vec![],
            cluster_by: vec![],
            distribute_by: vec![],
            into: None,
            qualify: None,
            sort_by: vec![],
        }));

        selects.push(body)
    }

    // Not the most elegant way of doing this but sufficient for now.
    let first = selects.remove(0);
    let body = selects
        .into_iter()
        .fold(first, |acc, select| SetExpr::SetOperation {
            op: sql_ast::SetOperator::Union,
            set_quantifier: sql_ast::SetQuantifier::All,
            left: Box::new(acc),
            right: Box::new(select),
        });

    sql_ast::Query {
        with: (None),
        body: Box::new(body),
        order_by: vec![],
        limit: None,
        offset: None,
        fetch: None,
        lock: None,
    }
}

#[cfg(test)]
mod test {

    use insta::assert_debug_snapshot;

    use super::*;

    #[test]
    fn test_csv() {
        let csv = "a,b,c\n1,2,3\n4,5,6";
        let data = data_of_csv(csv).unwrap();
        assert_debug_snapshot!(data, @r###"
        RelationLiteral {
            columns: [
                "a",
                "b",
                "c",
            ],
            rows: [
                [
                    "1",
                    "2",
                    "3",
                ],
                [
                    "4",
                    "5",
                    "6",
                ],
            ],
        }
        "###);

        assert_debug_snapshot!(sql_of_sample_data(&data), @r###""sample AS (SELECT 1 AS a, 2 AS b, 3 AS c UNION ALL SELECT 4 AS a, 5 AS b, 6 AS c)""###);
    }
}
