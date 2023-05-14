use std::{process, fs, path::PathBuf};

use biodivine_lib_bdd::{BddVariable, BddVariableSet};
use biodivine_lib_param_bn::BooleanNetwork;
use biodivine_lib_param_bn::symbolic_async_graph::
    {SymbolicContext, GraphColors};
use biodivine_lib_param_bn::biodivine_std::traits::Set;

use json::{JsonValue, object, array};
use clap::{Parser, Subcommand, Args};

use pbn_ibmfa::symbolic_sync_graph::SymbSyncGraph;
use pbn_ibmfa::utils::{add_self_regulations, variations_with_replacement};
use pbn_ibmfa::ibmfa_computations::ibmfa_entropy;
use pbn_ibmfa::driver_set::{find_driver_set, colors_partition, PBNFix, UnitFix,
    fixes::{DriverSet, UnitVertexFix}};



#[derive(Subcommand, Debug)]
enum Commands {
    /// Run analysis of a Parametrized Boolean Network.
    /// By default just detects fixed-point attractors.
    Analysis(AnalysisArgs),
    /// Run the simulation for average PBN dynamics.
    Simulation(SimulationArgs),
}

#[derive(Args, Debug)]
struct AnalysisArgs {
    /// Find a strong driver-set for each attractor.
    #[arg(short, long)]
    strong_dset: bool,
    /// Find an unconstrained strong driver-set.
    #[arg(short = 'S', long)]
    strong_dset_free: bool,
    /// Compute a partition of parametrizations given by driver-sets equality.
    #[arg(short, long)]
    driver_sets: bool,
    /// Do not reduce the driver-set.
    #[arg(long)]
    not_reduced: bool,
}

#[derive(Args, Debug)]
struct SimulationArgs {
    /// Compute the average dynamics by brute-force instead of IBMFA.
    #[arg(short, long)]
    brute_force: bool,
    /// Fix variable. Syntax: "{var_name}={value}". Value is "0" or "1".
    #[arg(short, long)]
    fix: Vec<String>,
}


/// A tool for running IBMFA on PBNs
#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Path to the input .aeon file
    path: std::path::PathBuf,
    /// Pretty json output
    #[arg(short, long)]
    pretty_json: bool,
    /// Length of the simulation
    #[arg(short, long, default_value_t = 10)]
    time_steps: u8,
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

fn load_model(path: &PathBuf) -> BooleanNetwork {
    let model_string = fs::read_to_string(path).unwrap_or_else(|err| {
        eprintln!("Cannot read the file, err: {}", err);
        process::exit(1);
    });
    let model = BooleanNetwork::try_from(model_string.as_str()).unwrap();
    add_self_regulations(model)
}

fn add_state_variables(
    context: &SymbolicContext,
    json_data: &mut json::JsonValue
) {
    let bdd_var_set = context.bdd_variable_set();
    json_data["state_variables"] = JsonValue::Array(
        context.state_variables().iter()
            .map(|bdd_var| JsonValue::String(bdd_var_set.name_of(*bdd_var)))
            .collect::<json::Array>()
    );
}

fn add_parameter_variables(
    context: &SymbolicContext,
    json_data: &mut json::JsonValue
) {
    let bdd_var_set = context.bdd_variable_set();
    json_data["parameter_variables"] = JsonValue::Array(
        context.parameter_variables().iter()
            .map(|bdd_var| JsonValue::String(bdd_var_set.name_of(*bdd_var)))
            .collect::<json::Array>()
    );
}

fn main_analysis(args: &Cli, analysis_args: &AnalysisArgs) {
    // Load the model from a file
    let model = load_model(&args.path);

    // Compute the symbolic synchronous transition graph
    let sync_graph = SymbSyncGraph::new(model);
    let context = sync_graph.symbolic_context();
    let bdd_var_set = context.bdd_variable_set();

    // Basic info about the model

    let mut json_data = json::JsonValue::new_object();

    add_state_variables(context, &mut json_data);
    add_parameter_variables(context, &mut json_data);

    json_data["colors"] = sync_graph.unit_colors().approx_cardinality().into();

    if analysis_args.strong_dset_free {
        let (pbn_fix, probs) = find_driver_set(
            &sync_graph, args.time_steps as usize,
            !analysis_args.not_reduced, None, true, false);

        let state = bdd_values_to_json(
            probs.iter().zip(context.state_variables().iter())
                .map(|(p, bdd_var)| (*bdd_var, *p == 1.0)),
            bdd_var_set
        );

        json_data["unconstrained"] = object!{
            attractor: state,
            strong_driver_set: driver_set_to_json(
                pbn_fix.get_driver_set(), context)
        };
    }

    // Compute the attractors
    let mut attrs = sync_graph.fixed_point_attractors();
    attrs.sort_by(|a1, a2| a2.exact_cardinality().cmp(&a1.exact_cardinality()));

    json_data["attractors"] = JsonValue::Array(attrs.iter()
        .map(|attr| {
            let state = bdd_values_to_json(
                attr.vertices().as_bdd().first_clause().unwrap().to_values(),
                bdd_var_set
            );

            let mut attr_json = object!{
                colors: attr.approx_cardinality(),
                state: state,
            };

            if !analysis_args.strong_dset && !analysis_args.driver_sets {
                return attr_json;
            }

            let attr_tuple = (&attr.vertices(), &attr.colors());

            // Strong driver-set
            if analysis_args.strong_dset {
                let (pbn_fix, _) = find_driver_set(
                    &sync_graph, args.time_steps as usize,
                    !analysis_args.not_reduced, Some(attr_tuple), true, false);

                attr_json["strong-driver-set"] = driver_set_to_json(
                    pbn_fix.get_driver_set(), context);
            }

            // Parametrizations partition by driver-set equality
            if analysis_args.driver_sets {
                let mut driver_sets = colors_partition(
                    &sync_graph, args.time_steps as usize,
                    !analysis_args.not_reduced, attr_tuple, false);
                driver_sets.sort_by(|(c1, _), (c2, _)|
                    c2.exact_cardinality().cmp(&c1.exact_cardinality()));
                let driver_sets = driver_sets.into_iter()
                    .map(|(colors, driver_set)| object!{
                        driver_set: driver_set_to_json(&driver_set, context),
                        colors: GraphColors::new(colors, context)
                            .approx_cardinality()
                    })
                    .collect::<json::Array>();

                attr_json["driver-sets"] = JsonValue::Array(driver_sets);
            }

            attr_json
        })
        .collect::<json::Array>()
    );

    print_json(json_data, args.pretty_json);
}


fn parse_fixes(fixes: &[String], model: &BooleanNetwork)
-> Result<Vec<UnitVertexFix>, String> {
    fixes.iter()
        .map(|fix| {
            let parts: Vec<&str> = fix.split('=').collect();
            if parts.len() != 2 {
                return Err(format!("Invalid fix '{fix}'. \
                    Expected format '{{name}}={{value}}'."));
            }
            let value =
                if parts[1] == "0" { false }
                else if parts[1] == "1" { true }
                else { return Err(format!("Invalid fix '{fix}'. \
                    Expected value '0'/'1', found '{}'.", parts[1]))
                };

            if let Some(var_id) = model.as_graph().find_variable(parts[0]) {
                Ok(UnitVertexFix { var_id, value })
            } else {
                Err(format!("Invalid fix '{fix}'. \
                    The model does not contain variable '{}'.", parts[0]))
            }
        })
        .collect()
}

fn main_simulation(args: &Cli, sim_args: &SimulationArgs) {
    let model = load_model(&args.path);
    let user_fixes = parse_fixes(&sim_args.fix, &model).unwrap_or_else(|err| {
        eprintln!("Err: {err}");
        process::exit(1);
    });
    let sync_graph = SymbSyncGraph::new(model);
    let context = sync_graph.symbolic_context();
    let bdd_var_set = context.bdd_variable_set();
    let mut pbn_fix = PBNFix::new(sync_graph.unit_colors().into_bdd());
    for unit_vertex_fix in user_fixes {
        pbn_fix.insert(&UnitFix::Vertex(unit_vertex_fix));
    }
    let fixes = pbn_fix.get_driver_set();

    let mut json_data = json::JsonValue::new_object();

    add_state_variables(context, &mut json_data);

    json_data["simulation"] = json::JsonValue::new_object();
    for var_id in sync_graph.as_network().variables() {
        let name = sync_graph.as_network().get_variable_name(var_id);
        json_data["simulation"][name] =
            if let Some(value) = fixes.get(&var_id) {
                array![if *value { 1.0 } else { 0.0 }]
            } else {
                array![0.5]
            };
    }

    let mut all_probs: Vec<Vec<f32>>;

    if sim_args.brute_force {
        let vars_num = context.num_state_variables();

        all_probs = vec![vec![0.0; vars_num]; args.time_steps as usize];

        let state_space =
            variations_with_replacement(&[0.0, 1.0], vars_num - fixes.len())
            .into_iter()
            .map(|mut variation| {
                let variables = sync_graph.as_network().variables();
                for (index, var_id) in variables.enumerate() {
                    if let Some(value) = fixes.get(&var_id) {
                        variation.insert(index, if *value { 1.0 } else { 0.0 });
                    }
                }
                variation
            })
            .collect::<Vec<_>>();

        let mut remaining_colors = sync_graph.unit_colors().clone();
        while !remaining_colors.is_empty() {
            let color = remaining_colors.pick_singleton();
            remaining_colors = remaining_colors.minus(&color);

            for state in &state_space {
                let mut iteration_probs = Vec::new();
                let mut add_probs = |probs: &[f32]| {
                    iteration_probs.push(probs.to_vec());
                };
                ibmfa_entropy(
                    &sync_graph,
                    &pbn_fix,
                    args.time_steps as usize,
                    false,
                    None,
                    Some(&mut add_probs),
                    Some(state.clone()),
                    false
                );
                for it in 0..args.time_steps as usize {
                    for i in 0..vars_num {
                        all_probs[it][i] += iteration_probs[it][i];
                    }
                }
            }
        }

        for it in 0..args.time_steps as usize {
            for i in 0..vars_num {
                all_probs[it][i] /= state_space.len() as f32
                    * sync_graph.unit_colors().approx_cardinality() as f32;
            }
        }

    } else {
        all_probs = Vec::new();
        let add_probs = |probs: &[f32]| {
            all_probs.push(probs.to_vec());
        };

        ibmfa_entropy(
            &sync_graph,
            &pbn_fix,
            args.time_steps as usize,
            false,
            None,
            Some(add_probs),
            None,
            false
        );
    }

    for iteration in all_probs.iter() {
        for (bdd_var, prob) in
                context.state_variables().iter().zip(iteration.iter()) {
            json_data["simulation"][bdd_var_set.name_of(*bdd_var)]
                .push(*prob).unwrap();
        }
    }

    print_json(json_data, args.pretty_json);
}

fn print_json(json_data: json::JsonValue, pretty: bool) {
    let json_str = if pretty {
        json::stringify_pretty(json_data, 4)
    } else {
        json::stringify(json_data)
    };
    println!("{}", json_str);
}

fn main() {
    let args = Cli::parse();
    match &args.command {
        Commands::Analysis(driver_args) => main_analysis(&args, driver_args),
        Commands::Simulation(sim_args) => main_simulation(&args, sim_args),
    }
}
