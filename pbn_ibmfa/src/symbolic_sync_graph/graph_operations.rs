use biodivine_lib_param_bn::{VariableId, symbolic_async_graph::
    {GraphColoredVertices, GraphColors}};

use super::SymbSyncGraph;

impl SymbSyncGraph {
    pub fn post_synch(&self, initial: &GraphColoredVertices)
    -> GraphColoredVertices {
        let output = initial.as_bdd() // (prev, ?)
            .and(&self.total_update_function) // (prev, next)
            .project(self.context.state_variables()) // (?, next)
            .and(&self.extra_state_var_equivalence) // (next, next)
            .project(self.context.all_extra_state_variables()); // (next, ?)
        GraphColoredVertices::new(output, &self.context)
    }

    pub fn pre_synch(&self, initial: &GraphColoredVertices)
    -> GraphColoredVertices {
        let output = initial.as_bdd() // (next, ?)
            .and(&self.extra_state_var_equivalence) // (next, next)
            .project(self.context.state_variables()) // (?, next)
            .and(&self.total_update_function) // (prev, next)
            .project(self.context.all_extra_state_variables()); // (prev, ?)
        GraphColoredVertices::new(output, &self.context)
    }

    pub fn unit_colored_vertices(&self) -> GraphColoredVertices {
        GraphColoredVertices::new(self.unit_bdd.clone(), &self.context)
    }

    pub fn empty_colored_vertices(&self) -> GraphColoredVertices {
        GraphColoredVertices::new(
            self.context.mk_constant(false), &self.context)
    }

    pub fn unit_colors(&self) -> GraphColors {
        GraphColors::new(self.unit_bdd.clone(), &self.context)
    }

    pub fn fix_network_variable(&self, variable: VariableId, value: bool)
    -> GraphColoredVertices {
        let bdd_var = self.context.get_state_variable(variable);
        GraphColoredVertices::new(
            self.unit_bdd.var_select(bdd_var, value), &self.context)
    }
}
