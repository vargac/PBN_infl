use std::cmp::Ordering;

use biodivine_lib_bdd::BddPartialValuation;

use crate::symbolic_sync_graph::{SymbSyncGraph, VarIndex, PUpdateFunExplicit};
use crate::driver_set::{PBNFix, UnitFix};


/// Computes the IBMFA for `sync_graph` fixed by `pbn_fix`.
///
/// Returns a tuple `(entropy, final_probabilities, last_iteration)`.
///
/// * `iterations` - Length of the simulation.
/// * `early_stop` - Stops the computation after two iterations of zero entropy.
/// * `explicit_pupdate_funs_opt` - Precomuted
///     `sync_graph.explicit_pupdate_functions()`. Speeds up the computation
///     when running more `ibmfa_entropy` still on the same network (the
///     valid parametrizations are the same)
/// * `step_callback_opt` - Called after every iteration, the parameter
///     is the current probabilities.
/// * `initial` - Initial configuration. By default deduced from fixed
///     variables in `pbn_fix`, others are put to 0.5.
/// * `verbose` - Print the probabilities after each step.
pub fn ibmfa_entropy(
    sync_graph: &SymbSyncGraph,
    pbn_fix: &PBNFix,
    iterations: usize,
    early_stop: bool,
    explicit_pupdate_funs_opt: Option<&[PUpdateFunExplicit]>,
    mut step_callback_opt: Option<impl FnMut(&[f32]) -> ()>,
    initial: Option<Vec<f32>>,
    verbose: bool,
) -> (f32, Vec<f32>, usize) {
    let mut probs = match initial {
        Some(initial) => initial,
        None => sync_graph.as_network().variables()
            .map(|var_id|
                if let Some(fixed_prob) = pbn_fix.get_vertex(var_id) {
                    if fixed_prob { 1.0 } else { 0.0 }
                } else {
                    0.5
                })
            .collect::<Vec<_>>(),
    };

    // In the case of finding color-fixes (not just vertex-fixes), now, it is
    // the time to compute explicit parametrizations of update functions,
    // as the update functions does not change any more (by means of restricting
    // some colors of the system).
    let explicit_pupdate_funs_binding = match explicit_pupdate_funs_opt {
        Some(_) => None,
        None => Some(sync_graph.explicit_pupdate_functions(&pbn_fix.colors())),
    };
    let explicit_pupdate_funs = explicit_pupdate_funs_opt.unwrap_or_else(
        || explicit_pupdate_funs_binding.as_ref().unwrap());

    let mut ent = 0.0;
    let mut last_ent = 1.0;
    for i in 0..iterations {
        probs =
            ibmfa_step(&sync_graph, &probs, &pbn_fix, explicit_pupdate_funs);
        if verbose {
            println!("{:?}", probs);
        }
        if let Some(ref mut step_callback) = step_callback_opt {
            step_callback(&probs);
        }
        ent = entropy(&probs);
        if ent == 0.0 && last_ent == 0.0 && early_stop {
            return (ent, probs, i - 1);
        }
        last_ent = ent;
    }
    (ent, probs, iterations)
}

/// Finds the minimizing fix for `sync_graph`.
///
/// Returns the found fix, entropy, and the corresponding network configuration.
///
/// * `iterations` - Length of the simulation.
/// * `pbn_fix` - Fix of the network. After the end of this function,
///     it remains in the same state as before.
/// * `available_fixes` - Find minimum of these.
/// * `explicit_pupdate_funs_opt` - As in `ibmfa_entropy`.
/// * `verbose` - Print entropies for fixes.
pub fn minimize_entropy<'a>(
    sync_graph: &SymbSyncGraph,
    iterations: usize,
    pbn_fix: &mut PBNFix,
    available_fixes: impl IntoIterator<Item = &'a UnitFix>,
    explicit_pupdate_funs_opt: Option<&[PUpdateFunExplicit]>,
    verbose: bool,
) -> Option<(&'a UnitFix, f32, Vec<f32>)> {
    available_fixes.into_iter()
        .map(|unit_fix| {
            if verbose {
                println!("Try fix {}",
                    unit_fix.to_str(&sync_graph.symbolic_context()));
            }

            pbn_fix.insert(unit_fix);
            let (ent, probs, index) = ibmfa_entropy(
                &sync_graph, &pbn_fix, iterations, true,
                explicit_pupdate_funs_opt, None::<fn(&[f32])>, None, false);
            pbn_fix.remove(unit_fix);

            if verbose {
                println!("{ent} at {index}");
            }

            (unit_fix, (index, ent), probs)
        })
        .min_by(|(_, a, _), (_, b, _)| match a.0.cmp(&b.0) {
            Ordering::Equal => a.1.total_cmp(&b.1),
            ord => ord,
        })
        .map(|(unit_fix, (_, ent), probs)| (unit_fix, ent, probs))
}


/********************
 * Helper functions *
 ********************/

fn entropy(probs: &[f32]) -> f32 {
    probs.iter()
        .map(|p| {
            let p = p.clamp(0.0, 1.0);
            if p == 0.0 || p == 1.0 { 0.0 }
                 else { - p * p.log2() - (1.0 - p) * (1.0 - p).log2() }
         })
        .sum::<f32>() / probs.len() as f32
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

fn ibmfa_step(
    sync_graph: &SymbSyncGraph,
    probs: &[f32],
    pbn_fix: &PBNFix,
    explicit_pupdate_funs: &[PUpdateFunExplicit],
) -> Vec<f32> {
    explicit_pupdate_funs.iter()
        .zip(sync_graph.as_network().variables())
        .map(|(f_parametrizations, var_id)|
            if let Some(fixed_prob) = pbn_fix.get_vertex(var_id) {
                if fixed_prob { 1.0 } else { 0.0 }
            } else {
                f_parametrizations.iter()
                    .map(|update_fun| update_fun
                        .sat_clauses()
                        .map(|clause| clause_probability(&clause, &probs,
                                sync_graph.get_var_index()))
                        .sum::<f32>())
                    .sum::<f32>() / f_parametrizations.len() as f32
            })
        .collect()
}
