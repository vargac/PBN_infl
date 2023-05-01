#![allow(unused_imports)]

use std::{env, process, fs};
use std::collections::{HashMap, HashSet};

use biodivine_lib_param_bn::{BooleanNetwork, FnUpdate};
use biodivine_lib_param_bn::biodivine_std::traits::Set;
use biodivine_lib_param_bn::symbolic_async_graph::
    {GraphColoredVertices, GraphColors, GraphVertices};
use biodivine_lib_bdd::{BddVariable, Bdd};

use pbn_ibmfa::symbolic_sync_graph::SymbSyncGraph;
use pbn_ibmfa::utils::{partial_valuation_to_str, valuation_to_str,
    vertices_to_str, attr_from_str, bdd_to_str, bdd_var_to_str,
    bdd_pick_unsupported, add_self_regulations};
use pbn_ibmfa::driver_set::{find_driver_set, fixes::DriverSet};
use pbn_ibmfa::decision_tree::{decision_tree, decision_tree_from_partition};


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
    let model = BooleanNetwork::try_from(model_string.as_str()).unwrap();

    // Add self regulation for input nodes
    // We don't want to fix them via color but via variable
    let model = add_self_regulations(model);

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

    let mut driver_sets_map = HashMap::new();
    for (attr, colors) in &attrs_map {

        let mut driver_sets: Vec<(Bdd, DriverSet)> = Vec::new();

        let mut remaining_colors = colors.clone();
        while !remaining_colors.is_empty() {
            let color = remaining_colors.pick_singleton();
            remaining_colors = remaining_colors.minus(&color);

            let (pbn_fix, _) = find_driver_set(
                &sync_graph, iterations, true, Some((&attr, &color)),
                true, false);
            assert!(pbn_fix.get_parameter_fixes().is_empty());

            let driver_set = pbn_fix.get_driver_set();
            if let Some(i) = driver_sets.iter()
                    .position(|(_, driver)| *driver == *driver_set) {
                driver_sets[i].0 = driver_sets[i].0.or(color.as_bdd());
            } else {
                driver_sets.push((color.into_bdd(), driver_set.clone()));
            }
        }

        let (global_pbn_fix, _) = find_driver_set(
            &sync_graph, iterations, true, Some((&attr, &colors)),
            true, false);
        let global_driver_set = global_pbn_fix.get_driver_set().clone();

        driver_sets_map.insert(attr, (driver_sets, global_driver_set));
    }

    let context = sync_graph.symbolic_context();
    for (attr, colors) in &attrs_map {
        let (driver_sets, global_driver_set) = &driver_sets_map[&attr];
        println!("{}", pbn_ibmfa::utils::vertices_to_str(attr, context));
        let dtree = decision_tree_from_partition(
            colors.as_bdd(),
            driver_sets.as_slice(),
            context.bdd_variable_set()
        );
        println!("{}", dtree.to_str(&context));

        /*
        for (colors, driver_set) in driver_sets {
            println!("{}: {}",
                colors.exact_cardinality(),
                pbn_ibmfa::driver_set::fixes::driver_set_to_str(
                    driver_set, context));
        }
        */
        println!("GLOBAL: {}",
            pbn_ibmfa::driver_set::fixes::driver_set_to_str(
                global_driver_set, context));
        println!();
    }

    /*
    let attr_arg = (&attr, &attrs_map[&attr]);
    find_reduced_driver_set(&sync_graph, iterations, Some(attr_arg), true);

    println!("Attractors: {}", attrs_map.len());
    for (i, (attr, colors)) in attrs_map.iter().enumerate() {
        println!("{i} (size {}): {}",
            colors.approx_cardinality(),
            vertices_to_str(attr, sync_graph.symbolic_context()));
        if attr.approx_cardinality() == 1.0 {
            let (pbn_fix, probs) = find_reduced_driver_set(
                &sync_graph, iterations, Some((&attr, &colors)), false);
            println!("{:?}", probs);
            if !pbn_fix.get_parameter_fixes().is_empty() {
                println!("<><><> FOUND <><><>");
            }
            println!("{}", pbn_fix.to_str(sync_graph.symbolic_context()));
        }
    }
    println!();
    */
}
