#![allow(unused_imports)]
#![allow(unused_mut)]

use std::{env, process, fs};
use std::collections::HashMap;

use biodivine_lib_param_bn::{BooleanNetwork, VariableId, FnUpdate};
use biodivine_lib_param_bn::symbolic_async_graph::SymbolicContext;
use biodivine_lib_bdd::{Bdd, BddVariable, BddPartialValuation};

mod _impl_regulation_constraint;

use crate::_impl_regulation_constraint::apply_regulation_constraints;


#[derive(Copy, Clone, Debug)]
struct Fixing {
    var_i: usize,
    value: bool,
}

#[derive(Clone, Debug)]
struct ParedUpdateFunction {
    function: Bdd,
    parametrizations: Bdd,
}

impl ParedUpdateFunction {
    fn new(update_function: &Bdd, unit_bdd: &Bdd) -> ParedUpdateFunction {
        let support_set = update_function.support_set();
        let mut parametrizations = unit_bdd.clone();
        for bdd_var in unit_bdd.support_set() {
            if !support_set.contains(&bdd_var) {
                parametrizations = parametrizations.var_project(bdd_var);
            }
        }
        ParedUpdateFunction {
            function: update_function.and(&parametrizations),
            parametrizations,
        }
    }
}

type VarIndex = HashMap<BddVariable, usize>;

fn clause_probability(
    clause: &BddPartialValuation,
    probs: &[f32],
    var_index: &VarIndex)
-> f32 {
    clause.to_values().iter()
        .map(|&(var, val)| {
            let prob_one = probs[var_index[&var]];
            if val { prob_one } else { 1.0 - prob_one }
        })
        .product()
}

fn ibmfa_step(
    pupdate_functions: &[ParedUpdateFunction],
    probs: &[f32],
    init_probs: &[f32],
    var_index: &HashMap<BddVariable, usize>)
-> Vec<f32> {
    pupdate_functions.iter()
        .enumerate()
        .map(|(i, pupdate_function)|
            if init_probs[i] == 0.0 || init_probs[i] == 1.0 {
                init_probs[i]
            } else {
                let mut pnumber = 0;
                pupdate_function.parametrizations
                    .sat_clauses()
                    .map(|parametrization| {
                        pnumber += 1;
                        pupdate_function.function
                            .restrict(&parametrization.to_values())
                            .sat_clauses()
                            .map(|clause|
                                clause_probability(&clause, &probs, &var_index))
                            .sum::<f32>()
                    })
 // TODO May be just count the number of parametrizations and iterate over all?
                    .sum::<f32>() / pnumber as f32
            })
        .collect()
}

fn ibmfa(pupdate_functions: &[ParedUpdateFunction], probs: &mut Vec<Vec<f32>>,
         var_index: &HashMap<BddVariable, usize>, verbose: bool) {
    for i in 1..probs.len() {
        probs[i] = ibmfa_step(&pupdate_functions, &probs[i - 1], &probs[0],
                              var_index);
        if verbose {
            println!("{:?}", probs[i]);
        }
    }
}

fn entropy(probs: &[f32]) -> f32 {
    probs.iter()
        .map(|p| if *p == 0.0 || *p == 1.0 { 0.0 }
                 else { - p * p.log2() - (1.0 - p) * (1.0 - p).log2() })
        .sum::<f32>() / probs.len() as f32
}

/* early_stop is not a good idea. May be rather simulate until the values
 * converge, up to max iteration number. */
fn ibmfa_entropy(
    pupdate_functions: &[ParedUpdateFunction],
    init_probs: &[f32],
    var_index: &HashMap<BddVariable, usize>,
    iterations: usize,
    early_stop: bool,
    verbose: bool)
-> f32 {
    let mut probs: Vec<f32> = init_probs.into();
    let mut ent = 0.0;
    for _ in 0..iterations {
        probs = ibmfa_step(&pupdate_functions, &probs, init_probs, var_index);
        if verbose {
            println!("{:?}", probs);
        }
        ent = entropy(&probs);
        if ent == 0.0 && early_stop {
            break;
        }
    }
    ent
}

fn valuation_to_str(valuation: &BddPartialValuation, context: &SymbolicContext)
-> String {
    format!("{:?}", valuation
        .to_values()
        .iter()
        .map(|&(bdd_var, val)| format!("{}={}",
                context.bdd_variable_set().name_of(bdd_var), val))
        .collect::<Vec<_>>()
    )
}

fn main() {
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
    println!("vars: {}, pars: {}", model.num_vars(), model.num_parameters());
    println!("vars: {:?}", model.variables()
        .map(|var_id| model.get_variable_name(var_id))
        .collect::<Vec<_>>()
    );
    println!();

    /*
    model.set_update_function(VariableId::from_index(4),
        FnUpdate::mk_var(VariableId::from_index(1))
            .negation()
            .or(FnUpdate::mk_var(VariableId::from_index(2)))
            .into()).unwrap();
    */


    let context = SymbolicContext::new(&model).unwrap();

    let mut bdd_var_index: VarIndex = HashMap::new();
    for var_id in model.variables() {
        bdd_var_index
            .insert(context.get_state_variable(var_id), var_id.to_index());
    }
    println!("{:?}", bdd_var_index);

    let update_functions: Vec<Bdd> = model
        .variables()
        .map(|variable| {
            let regulators = model.regulators(variable);
            model
                .get_update_function(variable)
                .as_ref()
                .map(|fun| context.mk_fn_update_true(fun))
                .unwrap_or_else(||
                    context.mk_implicit_function_is_true(variable, &regulators)
                )
        })
        .collect();

    let unit_bdd = apply_regulation_constraints(context.mk_constant(true),
                                                &model, &context).unwrap();

    let pared_update_functions = update_functions.iter()
        .map(|fun| ParedUpdateFunction::new(fun, &unit_bdd))
        .collect::<Vec<_>>();

    for var_id in model.variables() {
        let pupdate_function = &pared_update_functions[var_id.to_index()];
        for parametrization in pupdate_function.parametrizations.sat_clauses() {
            println!("{}", valuation_to_str(&parametrization, &context));
            let f = pupdate_function.function
                .restrict(&parametrization.to_values());
            println!("\t{}",
                f.to_boolean_expression(context.bdd_variable_set()));

            for valuation in f.sat_clauses() {
                println!("\t{}", valuation_to_str(&valuation, &context));
            }
        }
        println!();
    }


    let iterations = 10;
    let mut probs = vec![vec![0 as f32; model.num_vars()]; iterations + 1];
    probs[0] = vec![0.5; model.num_vars()];
//    probs[0][0] = 0.0;

    println!("Entropy: {}",
        ibmfa_entropy(&pared_update_functions, &probs[0], &bdd_var_index,
                      iterations, true, true));

    let mut available_fixings = Vec::new();
    for var_i in 0..model.num_vars() {
        available_fixings.push(Fixing { var_i, value: false });
        available_fixings.push(Fixing { var_i, value: true });
    }

    let mut driver_set = Vec::new();
    while available_fixings.len() > 0 {
        println!("======= {} ========", available_fixings.len());
        let (min_entropy_index, min_entropy) = available_fixings
            .iter()
            .map(|&fixing| {
                let mut init_probs = vec![0.5; model.num_vars()];
                init_probs[fixing.var_i] = if fixing.value { 1.0 } else { 0.0 };
                ibmfa_entropy(&pared_update_functions, &init_probs,
                              &bdd_var_index, iterations, false, false)
            })
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        let fixing = available_fixings.remove(min_entropy_index);
        available_fixings.remove(min_entropy_index / 2 * 2);
        println!("{fixing:?}, entropy: {min_entropy}");
        driver_set.push(fixing);
        if min_entropy == 0.0 {
            break;
        }
    }

    println!("{:?}", driver_set);
}
