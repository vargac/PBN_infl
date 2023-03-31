use biodivine_lib_bdd::BddPartialValuation;
use crate::{VarIndex, BNetwork, PBNFix};
use std::collections::HashMap;

pub(crate) fn clause_probability(
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

pub(crate) fn ibmfa_step(
    model: &BNetwork,
    probs: &[f32],
    pbn_fix: &PBNFix)
-> Vec<f32> {
    model.pupdate_functions.iter()
        .zip(model.bn.variables())
        .map(|(pupdate_function, var_id)|
            if let Some(fixed_prob) = pbn_fix.get_vertex(var_id) {
                if fixed_prob { 1.0 } else { 0.0 }
            } else {
                let mut pnumber = 0;
                pupdate_function
                    .restricted_parametrizations(&pbn_fix.colors_fix)
                    .sat_clauses()
                    .map(|parametrization| {
                        pnumber += 1;
                        pupdate_function.function
                            .restrict(&parametrization.to_values())
                            .sat_clauses()
                            .map(|clause| clause_probability(
                                    &clause, &probs, &model.var_index))
                            .sum::<f32>()
                    })
 // TODO May be just count the number of parametrizations and iterate over all?
                    .sum::<f32>() / pnumber as f32
            })
        .collect()
}

pub(crate) fn entropy(probs: &[f32]) -> f32 {
    probs.iter()
        .map(|p| if *p == 0.0 || *p == 1.0 { 0.0 }
                 else { - p * p.log2() - (1.0 - p) * (1.0 - p).log2() })
        .sum::<f32>() / probs.len() as f32
}

/* early_stop is not a good idea. May be rather simulate until the values
 * converge, up to max iteration number. */
pub(crate) fn ibmfa_entropy(
    model: &BNetwork,
    pbn_fix: &PBNFix,
    iterations: usize,
    early_stop: bool,
    verbose: bool)
-> (f32, Vec<f32>) {
    let mut probs = model.bn.variables()
        .map(|var_id|
            if let Some(fixed_prob) = pbn_fix.get_vertex(var_id) {
                if fixed_prob { 1.0 } else { 0.0 }
            } else {
                0.5
            })
        .collect::<Vec<_>>();
    let mut ent = 0.0;
    for _ in 0..iterations {
        probs = ibmfa_step(&model, &probs, &pbn_fix);
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
