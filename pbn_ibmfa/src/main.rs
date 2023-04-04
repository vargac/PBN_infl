#![allow(unused_imports)]
#![allow(unused_mut)]

use std::{env, process, fs};
use std::collections::{HashMap, HashSet};

use biodivine_lib_param_bn::BooleanNetwork;
use biodivine_lib_param_bn::biodivine_std::traits::Set;
use biodivine_lib_param_bn::symbolic_async_graph::
    {GraphColoredVertices, GraphColors, GraphVertices};

use symbolic_sync_graph::SymbSyncGraph;
use utils::{valuation_to_str, vertices_to_str, attr_from_str, bdd_to_str};
use driver_set::find_driver_set;


mod driver_set;
mod utils;
mod symbolic_sync_graph;
mod ibmfa_computations;


fn compute_attrs_map(attrs: &[GraphColoredVertices])
-> HashMap<GraphVertices, GraphColors> {
    let mut attrs_map = HashMap::new();
    for attr in attrs {
        let mut attr = attr.clone();
        while !attr.is_empty() {
            let mut wanted_vertices = attr
                .intersect_colors(&attr.colors().pick_singleton())
                .vertices();

            let one_attr_vertices = wanted_vertices.clone();

            let other_vertices = attr.vertices().minus(&one_attr_vertices);
            let mut one_attr_colors = attr
                .colors()
                .minus(&attr.intersect_vertices(&other_vertices).colors());

            while !wanted_vertices.is_empty() { // TODO just iterate over them
                let one_attr_vertex = wanted_vertices.pick_singleton();
                one_attr_colors = one_attr_colors.intersect(
                    &attr.intersect_vertices(&wanted_vertices).colors());
                wanted_vertices = wanted_vertices.minus(&one_attr_vertex);
            }

            attr = attr.minus_colors(&one_attr_colors);

            attrs_map
                .entry(one_attr_vertices)
                .and_modify(|colors: &mut GraphColors|
                    *colors = colors.union(&one_attr_colors))
                .or_insert(one_attr_colors);
        }
    }
    attrs_map
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
    let model = BooleanNetwork::try_from(model_string.as_str()).unwrap();

    // Print info about the model
    println!("vars: {}, pars: {}", model.num_vars(), model.num_parameters());
    println!("vars: {:?}", model.variables()
        .map(|var_id| model.get_variable_name(var_id))
        .collect::<Vec<_>>()
    );
    println!();

    // Compute the symbolic synchronous transition graph
    let sync_graph = SymbSyncGraph::new(model);

    // Print the parametrized update functions
    for pupdate_function in sync_graph.get_pupdate_functions() {
        let parametrizations = pupdate_function.get_parametrizations();
        for parametrization in parametrizations.sat_clauses() {
            println!("{}", valuation_to_str(
                    &parametrization, sync_graph.symbolic_context()));
            let f = pupdate_function.restricted(&parametrization);
            println!("\t{}", bdd_to_str(&f, sync_graph.symbolic_context()));

            for valuation in f.sat_clauses() {
                println!("\t{}", valuation_to_str(
                        &valuation, sync_graph.symbolic_context()));
            }
        }
        println!();
    }


    // Compute the strongest driver set in general
    let iterations = 10;
    let (pbn_fix, probs) = find_driver_set(&sync_graph, iterations, None, true);

    println!("Final:\n{}\n{:?}",
        pbn_fix.to_str(&sync_graph.symbolic_context()), probs);
    println!();

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
    let attrs = sync_graph.attractors();
    let attrs_map = compute_attrs_map(&attrs);

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
                println!("WRONG");
            }
            println!("{}", pbn_fix.to_str(sync_graph.symbolic_context()));
        }
    }
    println!();

    // Find a driver set for one specific attractor
    let attr = ["v_miR_9", "v_zic5"];
    let attr_vertices = attr_from_str(&attr, &sync_graph);

    println!("{}", vertices_to_str(
            &attr_vertices, sync_graph.symbolic_context()));
    println!("{}", bdd_to_str(
            attrs_map[&attr_vertices].as_bdd(), sync_graph.symbolic_context()));

    find_driver_set(
        &sync_graph,
        iterations,
        Some((&attr_vertices, &attrs_map[&attr_vertices])),
        true
    );
}
