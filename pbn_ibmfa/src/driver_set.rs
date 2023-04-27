use std::collections::HashSet;

use biodivine_lib_param_bn::biodivine_std::traits::Set;
use biodivine_lib_param_bn::symbolic_async_graph::{GraphColors, GraphVertices};
use biodivine_lib_bdd::Bdd;

use crate::symbolic_sync_graph::SymbSyncGraph;
use crate::ibmfa_computations::{minimize_entropy, ibmfa_entropy};
use fixes::{UnitVertexFix, UnitParameterFix};
pub use fixes::{PBNFix, UnitFix, driver_set_to_str};


pub mod fixes;


pub fn find_reduced_driver_set(
    sync_graph: &SymbSyncGraph,
    iterations: usize,
    attr_opt: Option<(&GraphVertices, &GraphColors)>,
    verbose: bool
) -> (PBNFix, Vec<f32>) {
    let (pbn_fix, probs) = find_driver_set(
        sync_graph, iterations, attr_opt, verbose);
    let pbn_fix = reduce_driver_set(pbn_fix, sync_graph, iterations, verbose);
    (pbn_fix, probs)
}

pub fn find_driver_set(
    sync_graph: &SymbSyncGraph,
    iterations: usize,
    attr_opt: Option<(&GraphVertices, &GraphColors)>,
    verbose: bool
) -> (PBNFix, Vec<f32>) {
    let colors = match attr_opt {
        Some((_, attr_colors)) => attr_colors.as_bdd().clone(),
        None => sync_graph.unit_colors().into_bdd(),
    };
    let (mut available_fixes, mut pbn_fix) =
        prepare_fixes(&sync_graph, attr_opt.map(|tup| tup.0), colors);

    let mut final_probs = Vec::new();

    while available_fixes.len() > 0 {
        if verbose {
            println!("======= {} ========", available_fixes.len());
        }

        let (unit_fix, min_entropy, probs) = minimize_entropy(
            &sync_graph, iterations, &mut pbn_fix, &available_fixes, verbose)
            .unwrap();

        pbn_fix.insert(unit_fix);

        if verbose {
            println!("Fixing {}, entropy:{min_entropy}",
                unit_fix.to_str(sync_graph.symbolic_context()));
            println!("{}", pbn_fix.to_str(sync_graph.symbolic_context()));
        }

        available_fixes = filter_fixes(&available_fixes, &pbn_fix);
        final_probs = probs;

        if min_entropy == 0.0 {
            break;
        }
    }
    (pbn_fix, final_probs)
}

pub fn reduce_driver_set(
    mut pbn_fix: PBNFix,
    sync_graph: &SymbSyncGraph,
    iterations: usize,
    verbose: bool
) -> PBNFix {
    let mut fixes = pbn_fix.get_driver_set()
        .iter()
        .map(|(&var_id, &value)|
            UnitFix::Vertex(UnitVertexFix { var_id, value }))
        .chain(pbn_fix.get_parameter_fixes()
            .iter()
            .map(|unit_par_fix| UnitFix::Parameter(unit_par_fix.clone())))
        .collect::<Vec<_>>();

    loop {
        let mut to_remove = None;
        let mut to_remove_i = 0;
        for (i, unit_fix) in fixes.iter().enumerate() {
            if verbose {
                println!("Try removing {}",
                    unit_fix.to_str(&sync_graph.symbolic_context()));
            }

            pbn_fix.remove(unit_fix);
            let (ent, _) = ibmfa_entropy(
                &sync_graph, &pbn_fix, iterations, false, false);
            pbn_fix.insert(unit_fix);

            if verbose {
                println!("{ent}");
            }

            if ent == 0.0 {
                to_remove = Some(unit_fix.clone());
                to_remove_i = i;
                break;
            }
        }
        if let Some(to_remove) = to_remove {
            pbn_fix.remove(&to_remove);
            fixes.remove(to_remove_i);
            if verbose {
                println!("Removing {}",
                    to_remove.to_str(&sync_graph.symbolic_context()));
                println!("{}", pbn_fix.to_str(sync_graph.symbolic_context()));
            }
        } else {
            break;
        }
    }

    pbn_fix
}

fn prepare_fixes(
    sync_graph: &SymbSyncGraph,
    attr_opt: Option<&GraphVertices>,
    colors: Bdd
) -> (Vec<UnitFix>, PBNFix) {
    let mut available_fixes = Vec::new();

    // Fixes of state variables
    for var_id in sync_graph.as_network().variables() {
        for value in [false, true] {
            if let Some(attr) = attr_opt.as_ref() {
                if attr.fix_network_variable(var_id, value).is_empty() {
                    continue;
                }
            }
            let fix = UnitVertexFix { var_id, value };
            available_fixes.push(UnitFix::Vertex(fix));
        }
    }

    // Fixes of parameter variables
    for bdd_var in sync_graph.symbolic_context().parameter_variables() {
        for value in [false, true] {
            let fix = UnitParameterFix { bdd_var: *bdd_var, value };
            available_fixes.push(UnitFix::Parameter(fix));
        }
    }

    let pbn_fix = PBNFix::new(colors);
    (filter_fixes(&available_fixes, &pbn_fix), pbn_fix)
}

fn filter_fixes(fixes: &[UnitFix], pbn_fix: &PBNFix) -> Vec<UnitFix> {
    // TODO benchmark more, if it really has a meaning for larger models
    let mut color_fixes = HashSet::new();
    fixes.iter()
        .filter(|fix| match fix {
            UnitFix::Parameter(UnitParameterFix { bdd_var, value }) => {
                let before = pbn_fix.colors();
                let after = before.var_select(*bdd_var, *value);
                !after.is_false() && after != before
                    && color_fixes.insert(after)
            },
            UnitFix::Vertex(UnitVertexFix { var_id, .. }) =>
                pbn_fix.get_vertex(*var_id).is_none()
        })
        .cloned()
        .collect()
}
