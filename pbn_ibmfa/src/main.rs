#![allow(unused_imports)]
#![allow(unused_mut)]

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
    bdd_pick_unsupported};
use pbn_ibmfa::driver_set::find_driver_set;
use pbn_ibmfa::decision_tree::decision_tree;


fn print_update_functions(sync_graph: &SymbSyncGraph) {
    for pupdate_function in sync_graph.get_pupdate_functions() {
        let parametrizations = pupdate_function.get_parametrizations();
        println!("{}", 
            bdd_to_str(&parametrizations, sync_graph.symbolic_context()));

        let pars = sync_graph.get_all_false()
            .project(pupdate_function.get_parameters())
            .and(parametrizations);

        for parametrization in pars.sat_valuations() {
            println!("Parametrization: {}", valuation_to_str(
                    &parametrization,
                    pupdate_function.get_parameters().iter().copied(),
                    sync_graph.symbolic_context()));
            let f = pupdate_function.restricted(&parametrization);
            println!("Update function: {}",
                bdd_to_str(&f, sync_graph.symbolic_context()));

            for valuation in f.sat_clauses() {
                println!("\t{}", partial_valuation_to_str(
                        &valuation, sync_graph.symbolic_context()));
            }
        }
        println!();
    }
}


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
    let mut model = BooleanNetwork::try_from(model_string.as_str()).unwrap();

    // Add self regulation for input nodes
    // We don't want to fix them via color but via variable
    for variable in model.variables() {
        if model.regulators(variable).is_empty()
        && model.get_update_function(variable).is_none() {
            let name = model.get_variable_name(variable);
            let regulation = format!("{} -> {}", name, name);
            model.as_graph_mut().add_string_regulation(&regulation).unwrap();
            model.add_update_function(
                variable, FnUpdate::Var(variable)).unwrap();
        }
    }

    // Print info about the model
    println!("vars: {}, pars: {}", model.num_vars(), model.num_parameters());
    println!("vars: {:?}", model.variables()
        .map(|var_id| model.get_variable_name(var_id))
        .collect::<Vec<_>>()
    );
    println!();

    // Compute the symbolic synchronous transition graph
    let sync_graph = SymbSyncGraph::new(model);

    /* TODO add as a test
    let init_state = vec![false, false, false, false, false, false];
    let start = init_state.iter()
        .zip(sync_graph.bn.variables())
        .fold(sync_graph.unit_colored_vertices(),
            |acc, (&val, var_id)| acc.fix_network_variable(var_id, val));

    println!("{}", sync_graph.bdd_to_str(
        sync_graph.pre_synch(&start).as_bdd()));
    println!("{}", sync_graph.bdd_to_str(start.as_bdd()));
    println!("{}", sync_graph.bdd_to_str(
        sync_graph.post_synch(&start).as_bdd()));
    println!();
    */

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
        if attr.approx_cardinality() == 1.0 {
            let (pbn_fix, probs) = find_driver_set(
                &sync_graph, iterations, Some((&attr, &colors)), false);
            println!("{:?}", probs);
            if !sync_graph.as_network()
                    .variables().enumerate().all(|(i, var_id)|
                        (probs[i] == 1.0 || probs[i] == 0.0)
                        && !attr.fix_network_variable(
                            var_id, probs[i] != 0.0).is_empty()) {
                println!("<><><> WRONG <><><>");
            }
            if !pbn_fix.get_parameter_fixes().is_empty() {
                println!("<><><> FOUND <><><>");
            }
            println!("{}", pbn_fix.to_str(sync_graph.symbolic_context()));
        }
    }
    println!();
}
