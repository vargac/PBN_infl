use std::{env, process, fs};
use std::collections::{HashMap, HashSet};

use biodivine_lib_param_bn::{BooleanNetwork, FnUpdate};
use biodivine_lib_param_bn::biodivine_std::traits::Set;
use biodivine_lib_param_bn::symbolic_async_graph::
    {GraphColoredVertices, GraphColors, GraphVertices};
use biodivine_lib_bdd::BddVariable;

use pbn_ibmfa::symbolic_sync_graph::SymbSyncGraph;
use pbn_ibmfa::utils::{partial_valuation_to_str, valuation_to_str,
    vertices_to_str, attr_from_str, bdd_to_str, bdd_var_to_str,
    bdd_pick_unsupported, add_self_regulations};
use pbn_ibmfa::driver_set::find_driver_set;
use pbn_ibmfa::decision_tree::decision_tree;


fn main() {
    // Load the model from a file
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Use with one parameter -- path to the .aeon model");
        process::exit(1);
    }
    let model_string = fs::read_to_string(&args[1]).unwrap_or_else(|err| {
        eprintln!("Cannot read the file, err: {}", err);
        process::exit(1);
    });
    let model = BooleanNetwork::try_from(model_string.as_str()).unwrap();
    let model = add_self_regulations(model.unwrap());

    // Print info about the model
    println!("vars: {}, pars: {}", model.num_vars(), model.num_parameters());
    println!("vars: {:?}", model.variables()
        .map(|var_id| model.get_variable_name(var_id))
        .collect::<Vec<_>>()
    );
    println!();

    // Compute the symbolic synchronous transition graph
    let sync_graph = SymbSyncGraph::new(model);

    // Compute the attractors
    let attrs = sync_graph.fixed_point_attractors();
    let attrs_map = attrs.iter()
        .map(|attr| (attr.vertices(), attr.colors()))
        .collect::<HashMap<GraphVertices, GraphColors>>();

    let iterations = 10;

    println!("Attractors: {}", attrs_map.len());
    for (i, (attr, colors)) in attrs_map.iter().enumerate() {
        println!("{i} (size {}): {}",
            colors.approx_cardinality(),
            vertices_to_str(attr, sync_graph.symbolic_context()));

        let dtree = decision_tree(&sync_graph, iterations, (&attr, &colors));
        println!("{}", dtree.to_str(sync_graph.symbolic_context()));
    }
}
