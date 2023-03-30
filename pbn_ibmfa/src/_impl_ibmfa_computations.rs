use biodivine_lib_bdd::BddPartialValuation;
use crate::{VarIndex, BNetwork, FixingItem};
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
    fixings: &HashMap<FixingItem, f32>)
-> Vec<f32> {
    let mut restriction = model.context.mk_constant(true);
    for (&item, &value) in fixings {
        if let FixingItem::Parameter(bdd_var) = item {
            let var_as_bdd = 
                if value == 1.0 { // TODO f32 -> bool
                    model.context.bdd_variable_set().mk_var(bdd_var)
                } else {
                    model.context.bdd_variable_set().mk_not_var(bdd_var)
                };
            restriction = restriction.and(&var_as_bdd);
        }
    }
    model.pupdate_functions.iter()
        .enumerate()
        .map(|(i, pupdate_function)|
            if let Some(&fixed_prob) = fixings.get(&FixingItem::Variable(i)) {
                fixed_prob
            } else {
                let mut pnumber = 0;
                pupdate_function.restricted_parametrizations(&restriction)
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
    fixings: &HashMap<FixingItem, f32>,
    iterations: usize,
    early_stop: bool,
    verbose: bool)
-> (f32, Vec<f32>) {
    let mut probs = (0..model.pupdate_functions.len())
        .map(|i|
            if let Some(&fixed_prob) = fixings.get(&FixingItem::Variable(i)) {
                fixed_prob
            } else {
                0.5
            })
        .collect::<Vec<_>>();
    let mut ent = 0.0;
    for _ in 0..iterations {
        probs = ibmfa_step(&model, &probs, &fixings);
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
