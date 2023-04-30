use biodivine_lib_bdd::BddPartialValuation;

use crate::symbolic_sync_graph::{SymbSyncGraph, VarIndex, PUpdateFunExplicit};
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
    explicit_pupdate_funs_opt: Option<&[PUpdateFunExplicit]>,
    verbose: bool,
) -> (f32, Vec<f32>) {
    let mut probs = sync_graph.as_network().variables()
        .map(|var_id|
            if let Some(fixed_prob) = pbn_fix.get_vertex(var_id) {
                if fixed_prob { 1.0 } else { 0.0 }
            } else {
                0.5
            })
        .collect::<Vec<_>>();

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
    for _ in 0..iterations {
        probs =
            ibmfa_step(&sync_graph, &probs, &pbn_fix, explicit_pupdate_funs);
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
            let (ent, probs) = ibmfa_entropy(
                &sync_graph, &pbn_fix, iterations, false,
                explicit_pupdate_funs_opt, false);
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
