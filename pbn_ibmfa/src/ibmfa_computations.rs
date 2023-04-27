use biodivine_lib_bdd::{BddPartialValuation, Bdd};

use crate::symbolic_sync_graph::{SymbSyncGraph, VarIndex};
use crate::driver_set::{PBNFix, UnitFix};


pub fn entropy(probs: &[f32]) -> f32 {
    probs.iter()
        .map(|p| if *p == 0.0 || *p == 1.0 { 0.0 }
                 else { - p * p.log2() - (1.0 - p) * (1.0 - p).log2() })
        .sum::<f32>() / probs.len() as f32
}

pub fn ibmfa_entropy(
    sync_graph: &SymbSyncGraph,
    pbn_fix: &PBNFix,
    iterations: usize,
    early_stop: bool,
    verbose: bool)
-> (f32, Vec<f32>) {
    let mut probs = sync_graph.as_network().variables()
        .map(|var_id|
            if let Some(fixed_prob) = pbn_fix.get_vertex(var_id) {
                if fixed_prob { 1.0 } else { 0.0 }
            } else {
                0.5
            })
        .collect::<Vec<_>>();
    let mut ent = 0.0;
    let parametrizations =
        precompute_parametrizations(sync_graph, pbn_fix);
    for _ in 0..iterations {
        probs = ibmfa_step(&sync_graph, &probs, &pbn_fix, &parametrizations);
        if verbose {
            println!("{:?}", probs);
        }
        ent = entropy(&probs);
        if ent == 0.0 && early_stop {
            break;
        }
    }
    (ent, probs)
}

// `pbn_fix` is `mut`, but after the function call it remains the same as
// before.
pub fn minimize_entropy<'a>(
    sync_graph: &SymbSyncGraph,
    iterations: usize,
    pbn_fix: &mut PBNFix,
    available_fixes: impl IntoIterator<Item = &'a UnitFix>,
    verbose: bool,
) -> Option<(&'a UnitFix, f32, Vec<f32>)> {
    available_fixes.into_iter()
        .map(|unit_fix| {
            if verbose {
                println!("Try fix {}",
                    unit_fix.to_str(&sync_graph.symbolic_context()));
            }

            pbn_fix.insert(unit_fix);
            let (ent, probs) = ibmfa_entropy(
                &sync_graph, &pbn_fix, iterations, false, false);
            pbn_fix.remove(unit_fix);

            if verbose {
                println!("{ent}");
            }

            (unit_fix, ent, probs)
        })
        .min_by(|(_, a, _), (_, b, _)| a.partial_cmp(b).unwrap())
}

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

enum UpdateFunData {
    Parametrization(Vec<Bdd>),
    Fixed(f32),
}

fn precompute_parametrizations(sync_graph: &SymbSyncGraph, pbn_fix: &PBNFix)
-> Vec<UpdateFunData> {
    let colors = pbn_fix.colors();
    sync_graph.get_pupdate_functions().iter()
        .zip(sync_graph.as_network().variables())
        .map(|(pupdate_function, var_id)| {
            if let Some(fixed_prob) = pbn_fix.get_vertex(var_id) {
                UpdateFunData::Fixed(if fixed_prob { 1.0 } else { 0.0 })
            } else {
                let parametrizations = pupdate_function
                    .restricted_parametrizations(colors.clone());

                let pars = sync_graph.get_all_false()
                    .project(pupdate_function.get_parameters())
                    .and(&parametrizations);

                UpdateFunData::Parametrization(pars
                    .sat_valuations()
                    .map(|parametrization|
                        pupdate_function.restricted(&parametrization))
                    .collect::<Vec<_>>()
                )
            }
        })
        .collect::<Vec<_>>()
}

fn ibmfa_step(
    sync_graph: &SymbSyncGraph,
    probs: &[f32],
    pbn_fix: &PBNFix,
    parametrizations: &Vec<UpdateFunData>,
) -> Vec<f32> {
    parametrizations.iter()
        .map(|update_fun_data| match update_fun_data {
            UpdateFunData::Fixed(value) => *value,
            UpdateFunData::Parametrization(f_parametrizations) =>
                f_parametrizations.iter()
                    .map(|update_fun| update_fun
                        .sat_clauses()
                        .map(|clause| clause_probability(&clause, &probs,
                                sync_graph.get_var_index()))
                        .sum::<f32>())
                    .sum::<f32>() / f_parametrizations.len() as f32
        }).collect()
}
