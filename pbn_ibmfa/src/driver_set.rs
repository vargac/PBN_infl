use biodivine_lib_param_bn::biodivine_std::traits::Set;
use biodivine_lib_param_bn::symbolic_async_graph::{GraphColors, GraphVertices};
use biodivine_lib_bdd::Bdd;

use crate::symbolic_sync_graph::SymbSyncGraph;
use crate::ibmfa_computations::ibmfa_entropy;
use fixes::{UnitFix, UnitVertexFix, UnitParameterFix};
pub use fixes::PBNFix;


mod fixes;


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

        let (min_entropy_index, min_entropy, probs) = minimize_entropy(
            &sync_graph, iterations, &mut pbn_fix, &available_fixes, verbose);

        let unit_fix = &available_fixes[min_entropy_index];
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
    fixes.iter()
        .filter(|fix| match fix {
            UnitFix::Parameter(UnitParameterFix { bdd_var, value }) => {
                let before = pbn_fix.colors();
                let after = before.var_select(*bdd_var, *value);
                !after.is_false() && after != before
            },
            UnitFix::Vertex(UnitVertexFix { var_id, .. }) =>
                pbn_fix.get_vertex(*var_id).is_none()
        })
        .cloned()
        .collect()
}

// `pbn_fix` is `mut`, but after the function call it remains the same as
// before.
fn minimize_entropy(
    sync_graph: &SymbSyncGraph,
    iterations: usize,
    pbn_fix: &mut PBNFix,
    available_fixes: &[UnitFix],
    verbose: bool,
) -> (usize, f32, Vec<f32>) {
    let (index, (entropy, probs)) = available_fixes.iter()
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

            (ent, probs)
        })
        .enumerate()
        .min_by(|(_, (a, _)), (_, (b, _))| a.partial_cmp(b).unwrap())
        .unwrap();
    (index, entropy, probs)
}
