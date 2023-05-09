use std::{process, fs, path::PathBuf};

use biodivine_lib_bdd::{BddVariable, BddVariableSet};
use biodivine_lib_param_bn::BooleanNetwork;
use biodivine_lib_param_bn::symbolic_async_graph::
    {SymbolicContext, GraphColors};

use json::{JsonValue, object, array};
use clap::{Parser, Subcommand, Args};

use pbn_ibmfa::symbolic_sync_graph::SymbSyncGraph;
use pbn_ibmfa::utils::add_self_regulations;
use pbn_ibmfa::ibmfa_computations::ibmfa_entropy;
use pbn_ibmfa::driver_set::{find_driver_set, colors_partition, PBNFix,
    fixes::DriverSet};



#[derive(Subcommand)]
enum Commands {
    /// Uses an IBMFA simulation to find driver sets for fixed point attractors
    /// by minimizing the entropy of the system.
    Driver(DriverArgs),
    /// Just run the IBMFA simulation
    Simulation(SimulationArgs),
}

#[derive(Args)]
struct DriverArgs {
    /// The path to the input .aeon file
    path: std::path::PathBuf,
    /// Do not reduce the driver set
    #[arg(long)]
    not_reduced: bool,
    /// Pretty json output
    #[arg(short, long)]
    pretty_json: bool,
}

#[derive(Args)]
struct SimulationArgs {
    /// The path to the input .aeon file
    path: std::path::PathBuf,
    /// The length of the simulation
    #[arg(short, long, default_value_t = 10)]
    t: u8,
    /// Pretty json output
    #[arg(short, long)]
    pretty_json: bool,
}


/// Run analysis of a Parametrized Boolean Network.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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

const ITERATIONS: usize = 10;

fn main_driver(args: &DriverArgs) {
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

    print_json(json_data, args.pretty_json);
}

fn main_simulation(args: &SimulationArgs) {
    let model = load_model(&args.path);
    let sync_graph = SymbSyncGraph::new(model);
    let context = sync_graph.symbolic_context();
    let bdd_var_set = context.bdd_variable_set();

    let mut json_data = json::JsonValue::new_object();

    add_state_variables(context, &mut json_data);

    json_data["simulation"] = json::JsonValue::new_object();
    for bdd_var in context.state_variables().iter() {
        json_data["simulation"][bdd_var_set.name_of(*bdd_var)] = array![0.5];
    }

    let add_probs = |probs: &[f32]| {
        for (bdd_var, prob) in
                context.state_variables().iter().zip(probs.iter()) {
            json_data["simulation"][bdd_var_set.name_of(*bdd_var)]
                .push(*prob).unwrap();
        }
    };

    ibmfa_entropy(
        &sync_graph,
        &PBNFix::new(sync_graph.unit_colors().into_bdd()),
        args.t as usize,
        false,
        None,
        Some(add_probs),
        false
    );

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
        Commands::Driver(args) => main_driver(args),
        Commands::Simulation(args) => main_simulation(args),
    }
}
