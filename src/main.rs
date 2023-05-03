use adapter::GitlabAdapter;
use gitlab::types::Project;
use gitlab::{
    api::{
        projects::{Projects, ProjectsBuilder},
        Client, Query,
    },
    Gitlab, GitlabBuilder,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use trustfall::{execute_query, FieldValue, Schema, TransparentValue};

pub mod adapter;
pub mod vertex;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref SCHEMA: Schema = Schema::parse(include_str!("schema.graphql")).unwrap();
}

#[derive(Debug, Clone, Deserialize)]
struct InputQuery<'a> {
    query: &'a str,

    args: BTreeMap<Arc<str>, FieldValue>,
}

fn main() {
    let mut args = BTreeMap::new();
    // args.insert(
    //     Arc::from(String::from("ref")),
    //     FieldValue::String("master".to_string()),
    // );

    let input_query: InputQuery = InputQuery {
        query: include_str!("query.graphql"),
        args,
    };

    let adapter = Rc::new(GitlabAdapter::new());

    let query = input_query.query;
    let arguments = input_query.args;

    for data_item in execute_query(&SCHEMA, adapter, query, arguments)
        .expect("not a legal query")
        .take(10)
    {
        // The default `FieldValue` JSON representation is explicit about its type, so we can get
        // reliable round-trip serialization of types tricky in JSON like integers and floats.
        //
        // The `TransparentValue` type is like `FieldValue` minus the explicit type representation,
        // so it's more like what we'd expect to normally find in JSON.
        let transparent: BTreeMap<_, TransparentValue> =
            data_item.into_iter().map(|(k, v)| (k, v.into())).collect();
        println!("\n{}", serde_json::to_string_pretty(&transparent).unwrap());
    }
}
