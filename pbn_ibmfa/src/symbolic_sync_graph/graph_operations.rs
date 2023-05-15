use biodivine_lib_param_bn::{VariableId, symbolic_async_graph::
    {GraphColoredVertices, GraphColors}};

use super::SymbSyncGraph;

impl SymbSyncGraph {
    /// Returns the succesors of `inital` in synchronuous semantics
    pub fn post_synch(&self, initial: &GraphColoredVertices)
    -> GraphColoredVertices {
        let output = initial.as_bdd() // (prev, ?)
            .and(&self.total_update_function) // (prev, next)
            .project(self.context.state_variables()) // (?, next)
            .and(&self.extra_state_var_equivalence) // (next, next)
            .project(self.context.all_extra_state_variables()); // (next, ?)
        GraphColoredVertices::new(output, &self.context)
    }

    /// Returns the predecesors of `inital` in synchronuous semantics
    pub fn pre_synch(&self, initial: &GraphColoredVertices)
    -> GraphColoredVertices {
        let output = initial.as_bdd() // (next, ?)
            .and(&self.extra_state_var_equivalence) // (next, next)
            .project(self.context.state_variables()) // (?, next)
            .and(&self.total_update_function) // (prev, next)
            .project(self.context.all_extra_state_variables()); // (prev, ?)
        GraphColoredVertices::new(output, &self.context)
    }

    /// Returns all vertices and valid parametrizations
    pub fn unit_colored_vertices(&self) -> GraphColoredVertices {
        GraphColoredVertices::new(self.unit_bdd.clone(), &self.context)
    }

    /// Returns empty vertices and parametrizations
    pub fn empty_colored_vertices(&self) -> GraphColoredVertices {
        GraphColoredVertices::new(
            self.context.mk_constant(false), &self.context)
    }

    /// Returns valid parametrizations
    pub fn unit_colors(&self) -> GraphColors {
        GraphColors::new(self.unit_bdd.clone(), &self.context)
    }

    /// Returns all vertices and valid parametrizations but with vertex
    /// `variable` set to `value`.
    pub fn fix_network_variable(&self, variable: VariableId, value: bool)
    -> GraphColoredVertices {
        let bdd_var = self.context.get_state_variable(variable);
        GraphColoredVertices::new(
            self.unit_bdd.var_select(bdd_var, value), &self.context)
    }
}
