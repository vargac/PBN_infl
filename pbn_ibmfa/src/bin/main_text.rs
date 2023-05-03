use std::{process, fs};
use std::collections::HashMap;

use biodivine_lib_bdd::{BddVariable, BddVariableSet};
use biodivine_lib_param_bn::BooleanNetwork;
use biodivine_lib_param_bn::symbolic_async_graph::
    {SymbolicContext, GraphColors};

use json::{JsonValue, object};
use clap::Parser;

use pbn_ibmfa::symbolic_sync_graph::SymbSyncGraph;
use pbn_ibmfa::utils::add_self_regulations;
use pbn_ibmfa::decision_tree::decision_tree;
use pbn_ibmfa::driver_set::{find_driver_set, colors_partition,
    fixes::DriverSet};


/// Run analysis of a Parametrized Boolean Network.
/// Uses an IBMFA simulation to find driver sets for fixed point attractors
/// by minimizing the entropy of the system.
#[derive(Parser)]
struct Cli {
    /// The path to the input .aeon file
    path: std::path::PathBuf,
    /// Do not reduce the driver set
    #[arg(long)]
    not_reduced: bool,
    /// Pretty json output
    #[arg(short, long)]
    pretty_json: bool,
}


fn bdd_values_to_json(
    values: impl IntoIterator<Item = (BddVariable, bool)>,
    bdd_var_set: &BddVariableSet
) -> JsonValue {
    JsonValue::Array(values.into_iter()
        .map(|(bdd_var, value)| object!{
            variable: bdd_var_set.name_of(bdd_var),
            value: value})
        .collect::<json::Array>()
    )
}

fn driver_set_to_json(driver_set: &DriverSet, context: &SymbolicContext)
-> JsonValue {
    bdd_values_to_json(
        driver_set.iter().map(|(var_id, value)|
            (context.get_state_variable(*var_id), *value)),
        context.bdd_variable_set()
    )
}

const ITERATIONS: usize = 10;

fn main() {
    // Load the model from a file
    let args = Cli::parse();
    let model_string = fs::read_to_string(args.path).unwrap_or_else(|err| {
        eprintln!("Cannot read the file, err: {}", err);
        process::exit(1);
    });
    let model = BooleanNetwork::try_from(model_string.as_str()).unwrap();
    let model = add_self_regulations(model);

    // Compute the symbolic synchronous transition graph
    let sync_graph = SymbSyncGraph::new(model);
    let context = sync_graph.symbolic_context();
    let bdd_var_set = context.bdd_variable_set();

    // Basic info about the model

    let mut json_data = json::JsonValue::new_object();

    json_data["state_variables"] = JsonValue::Array(
        context.state_variables().iter()
            .map(|bdd_var| JsonValue::String(bdd_var_set.name_of(*bdd_var)))
            .collect::<json::Array>()
    );
    json_data["parameter_variables"] = JsonValue::Array(
        context.parameter_variables().iter()
            .map(|bdd_var| JsonValue::String(bdd_var_set.name_of(*bdd_var)))
            .collect::<json::Array>()
    );

    json_data["colors"] = sync_graph.unit_colors().approx_cardinality().into();


    // Compute the attractors
    let mut attrs = sync_graph.fixed_point_attractors();
    attrs.sort_by(|a1, a2| a2.exact_cardinality().cmp(&a1.exact_cardinality()));

    json_data["attractors"] = JsonValue::Array(attrs.iter()
        .map(|attr| {
            let state = bdd_values_to_json(
                attr.vertices().as_bdd().first_clause().unwrap().to_values(),
                bdd_var_set
            );

            let attr_tuple = (&attr.vertices(), &attr.colors());

            let (pbn_fix, _) = find_driver_set(
                &sync_graph, ITERATIONS, !args.not_reduced,
                Some(attr_tuple), true, false);

            let global_driver_set = driver_set_to_json(
                pbn_fix.get_driver_set(), context);

            let mut driver_sets = colors_partition(
                &sync_graph, ITERATIONS, !args.not_reduced, attr_tuple, false);
            driver_sets.sort_by(|(c1, _), (c2, _)|
                c2.exact_cardinality().cmp(&c1.exact_cardinality()));
            let driver_sets = driver_sets.into_iter()
                .map(|(colors, driver_set)| object!{
                    driver_set: driver_set_to_json(&driver_set, context),
                    colors: GraphColors::new(colors, context)
                        .approx_cardinality()
                })
                .collect::<json::Array>();

            object!{
                colors: attr.approx_cardinality(),
                state: state,
                global_driver_set: global_driver_set,
                driver_sets: JsonValue::Array(driver_sets)
            }})
        .collect::<json::Array>()
    );

    let json_str = if args.pretty_json {
        json::stringify_pretty(json_data, 4)
    } else {
        json::stringify(json_data)
    };
    println!("{}", json_str);
}
