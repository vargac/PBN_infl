use biodivine_lib_bdd::BddPartialValuation;

use crate::symbolic_sync_graph::{SymbSyncGraph, VarIndex};
use crate::driver_set::{PBNFix, UnitFix};
use crate::utils::bdd_pick_unsupported;


pub fn entropy(probs: &[f32]) -> f32 {
    probs.iter()
        .map(|p| if *p == 0.0 || *p == 1.0 { 0.0 }
                 else { - p * p.log2() - (1.0 - p) * (1.0 - p).log2() })
        .sum::<f32>() / probs.len() as f32
}

/* TODO: early_stop is not a good idea. May be rather simulate until the values
 * converge, up to max iteration number. */
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
    for _ in 0..iterations {
        probs = ibmfa_step(&sync_graph, &probs, &pbn_fix);
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
    var_index: &VarIndex) // TODO iterovat radsej cez state variables
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
    pbn_fix: &PBNFix)
-> Vec<f32> {
    sync_graph.get_pupdate_functions().iter()
        .zip(sync_graph.as_network().variables())
        .map(|(pupdate_function, var_id)|
            if let Some(fixed_prob) = pbn_fix.get_vertex(var_id) {
                if fixed_prob { 1.0 } else { 0.0 }
            } else {
                let mut pnumber = 0;
                let parametrizations = pupdate_function
                    .restricted_parametrizations(pbn_fix.colors());

                let pars = sync_graph.get_all_false()
                    .project(pupdate_function.get_parameters())
                    .and(&parametrizations);

                pars.sat_valuations()
                    .map(|parametrization| {
                        pnumber += 1;
                        pupdate_function.restricted(&parametrization)
                            .sat_clauses()
                            .map(|clause| clause_probability(&clause, &probs,
                                    sync_graph.get_var_index()))
                            .sum::<f32>()
                    })
                    .sum::<f32>() / pnumber as f32
            })
        .collect()
}
