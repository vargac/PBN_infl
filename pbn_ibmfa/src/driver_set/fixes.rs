use std::collections::{HashMap, HashSet};

use biodivine_lib_bdd::{Bdd, BddVariable};
use biodivine_lib_param_bn::{VariableId,
    symbolic_async_graph::{SymbolicContext}};

use crate::utils::{bdd_to_str, bdd_var_to_str};


#[derive(Debug, Clone)]
pub struct UnitVertexFix {
    pub var_id: VariableId,
    pub value: bool,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct UnitParameterFix {
    pub bdd_var: BddVariable,
    pub value: bool,
}

#[derive(Debug, Clone)]
pub enum UnitFix {
    Vertex(UnitVertexFix),
    Parameter(UnitParameterFix),
}

pub type DriverSet = HashMap<VariableId, bool>;
pub type ColorsFix = Bdd;

#[derive(Debug, Clone)]
pub struct PBNFix {
    driver_set: DriverSet,
    colors_fix: ColorsFix,
    parameter_fixes: HashSet<UnitParameterFix>,
    unit_bdd: ColorsFix,
}


pub fn driver_set_to_str(
    driver_set: &DriverSet,
    context: &SymbolicContext)
-> String {
    format!("{{ {}}}", driver_set.iter()
        .map(|(&var_id, &value)| format!("{} ",
            (UnitVertexFix { var_id, value }).to_str(&context)))
        .collect::<String>())
}

impl UnitVertexFix {
    pub fn to_str(&self, context: &SymbolicContext) -> String {
        let bdd_var = context.get_state_variable(self.var_id);
        format!("{}={}",
            bdd_var_to_str(bdd_var, &context),
            if self.value { 1 } else { 0 })
    }
}

impl UnitParameterFix {
    pub fn to_str(&self, context: &SymbolicContext) -> String {
        format!("{}={}",
            bdd_var_to_str(self.bdd_var, &context),
            if self.value { 1 } else { 0 })
    }
}

impl UnitFix {
    pub fn to_str(&self, context: &SymbolicContext) -> String {
        match self {
            UnitFix::Vertex(fix) => fix.to_str(&context),
            UnitFix::Parameter(fix) => fix.to_str(&context),
        }
    }
}

impl PBNFix {
    pub fn new(unit_bdd: Bdd) -> Self {
        PBNFix {
            driver_set: HashMap::new(),
            colors_fix: unit_bdd.iff(&unit_bdd), // hack to create true bdd
            parameter_fixes: HashSet::new(),
            unit_bdd,
        }
    }

    pub fn get_colors_fix(&self) -> &Bdd {
        &self.colors_fix
    }

    pub fn clear_colors_fix(&mut self) {
        self.parameter_fixes.clear();
        self.colors_fix = self.colors_fix.iff(&self.colors_fix);
    }

    pub fn colors(&self) -> ColorsFix {
        self.unit_bdd.and(&self.colors_fix)
    }

    pub fn get_parameter_fixes(&self) -> &HashSet<UnitParameterFix> {
        &self.parameter_fixes
    }

    pub fn get_vertex(&self, vertex: VariableId) -> Option<bool> {
        self.driver_set.get(&vertex).copied()
    }

    pub fn get_driver_set(&self) -> &DriverSet {
        &self.driver_set
    }

    pub fn insert(&mut self, fix: &UnitFix) {
        match fix {
            UnitFix::Vertex(fix) =>
                if self.driver_set.insert(fix.var_id, fix.value).is_some() {
                    panic!("Overriding driver-set by {:?}", fix);
                }
            UnitFix::Parameter(fix) => {
                self.colors_fix =
                    self.colors_fix.var_select(fix.bdd_var, fix.value);
                self.parameter_fixes.insert(fix.clone());
            }
        }
    }

    pub fn remove(&mut self, fix: &UnitFix) {
        match fix {
            UnitFix::Vertex(fix) =>
                if self.driver_set.remove(&fix.var_id).is_none() {
                    panic!("Fix not in the driver-set {:?}", fix);
                }
            UnitFix::Parameter(fix) => {
                if !self.parameter_fixes.remove(fix) {
                    panic!("Fix not among the parameter-fixes {:?}", fix);
                }
                self.colors_fix = self.colors_fix.var_project(fix.bdd_var);
            }
        }
    }


    pub fn par_fixes_to_str(&self, context: &SymbolicContext) -> String {
        format!("{{ {}}}", self.parameter_fixes.iter()
            .map(|par_fix| format!("{} ", par_fix.to_str(&context)))
            .collect::<String>())
    }

    pub fn colors_to_str(&self, context: &SymbolicContext) -> String {
        bdd_to_str(&self.colors(), &context)
    }

    pub fn to_str(&self, context: &SymbolicContext) -> String {
        format!("Driver-set: {}\nParameter-fixes: {}",
            driver_set_to_str(&self.driver_set, &context),
            self.par_fixes_to_str(&context))
    }
}
