// TODO benchmarking!!!
extern crate websocket;

use std::collections::HashMap;

use pbn_ibmfa::utils::{add_self_regulations, bdd_to_str};
use pbn_ibmfa::symbolic_sync_graph::SymbSyncGraph;
use pbn_ibmfa::driver_set::{colors_partition, find_driver_set,
    fixes::DriverSet};
use pbn_ibmfa::decision_tree::{DecisionTree, decision_tree_from_partition};

use biodivine_lib_bdd::BddVariableSet;

use biodivine_lib_param_bn::{BooleanNetwork,
    symbolic_async_graph::{GraphColoredVertices, GraphColors, SymbolicContext}};

use websocket::{sync::{Server, Client, Stream}, OwnedMessage};


const ITERATIONS: usize = 10;

struct SessionData {
    sync_graph: Option<SymbSyncGraph>,
    attrs: Option<Vec<GraphColoredVertices>>,
    cache: HashMap<usize, (DecisionTree, DriverSet)>,
}

impl SessionData {
    fn new() -> Self {
        SessionData {
            sync_graph: None,
            attrs: None,
            cache: HashMap::new(),
        }
    }
}


fn open_model(data: &[u8]) -> Result<BooleanNetwork, String> {
    match std::str::from_utf8(data) {
        Ok(model_str) => BooleanNetwork::try_from(model_str),
        Err(_) => Err("Cannot read the file".into()),
    }
}

fn attrs_to_msg(attrs: &[GraphColoredVertices], context: &SymbolicContext)
-> OwnedMessage {
    let mut msg_str = attrs.iter()
        .map(|attr| {
            let valuation = attr.vertices().as_bdd().first_valuation().unwrap();
            let bitstring = context.state_variables()
                .iter()
                .map(|var_id| if valuation[*var_id] { '1' } else { '0' })
                .collect::<String>();
            (attr.colors().exact_cardinality(), bitstring)})
        .fold(String::from(""), |mut acc, (cardinality, bitstring)| {
            acc.push_str(&cardinality.to_str_radix(10));
            acc.push(' ');
            acc.push_str(&bitstring);
            acc.push(' ');
            acc});
    msg_str.pop();
    OwnedMessage::Text(msg_str)
}

fn tree_and_dset_to_msg(
    tree: &DecisionTree,
    dset: &DriverSet,
    colors: &GraphColors,
    sync_graph: &SymbSyncGraph
) -> OwnedMessage {
    let mut buffer = String::new();
    driver_set_to_str(dset, sync_graph.symbolic_context(), &mut buffer);
    buffer.push(' ');
    tree_to_str_rec(tree, colors, sync_graph, &mut buffer);
    OwnedMessage::Text(buffer)
}

fn driver_set_to_str(
    driver_set: &DriverSet,
    context: &SymbolicContext,
    out: &mut String,
) {
    let bdd_var_set = context.bdd_variable_set();
    out.push('[');
    for (var_id, val) in driver_set {
        out.push(' ');
        out.push_str(&bdd_var_set.name_of(
                context.get_state_variable(*var_id)));
        out.push('=');
        out.push(if *val { '1' } else { '0' });
    }
    out.push_str(" ]");
}

fn tree_to_str_rec(
    tree: &DecisionTree,
    colors: &GraphColors,
    sync_graph: &SymbSyncGraph,
    out: &mut String
) {
    let context = sync_graph.symbolic_context();
    let bdd_var_set = context.bdd_variable_set();
    match tree {
        DecisionTree::Leaf(driver_set) => {
            driver_set_to_str(driver_set, context, out);
        },
        DecisionTree::Node(node) => {
            let fix_bdd = node.get_fix();
            let fix_false = colors.copy(colors.as_bdd().and_not(fix_bdd));
            let fix_true = colors.copy(colors.as_bdd().and(fix_bdd));

            // Add context for the fixing parameter variables
            for (bdd_var, val) in fix_bdd.first_clause().unwrap().to_values() {
                let fix_name = bdd_var_set.name_of(bdd_var);
                let name = contextify_fix_name(&fix_name, sync_graph);
                if !val {
                    out.push('!');
                }
                out.push_str(&name);
                out.push(';');
            }
            out.pop();

            out.push(' ');
            out.push_str(&fix_false.exact_cardinality().to_str_radix(10));
            out.push(' ');
            out.push_str(&fix_true.exact_cardinality().to_str_radix(10));
            out.push(' ');
            tree_to_str_rec(&node.get_childs()[0], &fix_false, sync_graph, out);
            out.push(' ');
            tree_to_str_rec(&node.get_childs()[1], &fix_true, sync_graph, out);
        }
    }
}

// f_{target name}[{regulators}]
// => {target name}({0/1{name} for each regulator})
// else return the same as input
fn contextify_fix_name(fix_name: &str, sync_graph: &SymbSyncGraph) -> String {
    let mut fix_opt: Option<String> = None;

    if fix_name.starts_with("f_") {
        if let Some(index) = fix_name.find('[') {
            let name = &fix_name[2..index];
            let args = &fix_name[index + 1..fix_name.len() - 1];
            let reg_graph = sync_graph.as_network().as_graph();
            if let Some(var_id) = reg_graph.find_variable(name) {
                let mut result = sync_graph.as_network()
                    .regulators(var_id)
                    .iter()
                    .zip(args.split(','))
                    .fold(format!("{name}("), |mut acc, (reg_id, val)| {
                        acc.push_str(val);
                        acc.push_str(sync_graph.as_network()
                            .get_variable_name(*reg_id));
                        acc.push(',');
                        acc
                    });
                result.pop();
                result.push(')');
                fix_opt = Some(result);
            }
        }
    }

    if let Some(fix) = fix_opt { fix } else { fix_name.into() }
}

fn get_response(msg: OwnedMessage, session_data: &mut SessionData)
-> Result<OwnedMessage, String> {
    let sync_graph = session_data.sync_graph.as_ref().unwrap();
    let context = sync_graph.symbolic_context();
    match msg {
        OwnedMessage::Text(msg) => {
            println!("Command {msg}");
            if msg == "START" {
                let mut attrs = sync_graph.fixed_point_attractors();
                attrs.sort_by(|a1, a2| a2.exact_cardinality()
                    .cmp(&a1.exact_cardinality())); // descending
                let msg = attrs_to_msg(&attrs, context);
                session_data.attrs = Some(attrs);
                Ok(msg)
            } else if msg.starts_with("TREE ") {
                match &session_data.attrs {
                    None => Err(format!("Error: '{msg:?}' before attractors")),
                    Some(attrs) => {
                        let id = msg.rsplit(' ').next().unwrap()
                            .parse::<usize>().unwrap();
                        let (dtree, dset) = session_data.cache
                            .entry(id)
                            .or_insert_with(|| {
                                let attr = &attrs[id];
                                let attr = (&attr.vertices(), &attr.colors());
                                let driver_sets = colors_partition(
                                    sync_graph, ITERATIONS, true, attr, false);
                                let dtree = decision_tree_from_partition(
                                    attr.1.as_bdd(),
                                    driver_sets.as_slice(),
                                    context.bdd_variable_set()
                                );
                                let (pbn_fix, _) = find_driver_set(
                                    sync_graph, ITERATIONS,
                                    true, Some(attr), true, false
                                );
                                (dtree, pbn_fix.get_driver_set().clone())
                            });
                        Ok(tree_and_dset_to_msg(
                            &dtree, &dset, &attrs[id].colors(), sync_graph))
                    }
                }
            } else {
                Err(format!("Error: unexpected command '{msg:?}'"))
            }
        },
        _ => Err(format!("Error: unexpected message type '{msg:?}'")),
    }
}

fn session_loop<S: Stream>(
    client: &mut Client<S>,
    session_data: &mut SessionData
) -> bool {
    let msg = client.recv_message().unwrap();
    match msg {
        // New model
        OwnedMessage::Binary(vec) => match open_model(&vec) {
            Ok(model) => {
                println!("New session");
                let sync_graph =
                    SymbSyncGraph::new(add_self_regulations(model));
                let model = sync_graph.as_network();
                let colors_num = sync_graph.unit_colors()
                    .exact_cardinality().to_str_radix(10);

                let msg_str = model.variables()
                    .map(|var_id| model.get_variable_name(var_id))
                    .fold(format!("OK {colors_num}"), |mut acc, name| {
                        acc.push(' ');
                        acc.push_str(name);
                        acc});
                client.send_message(
                    &OwnedMessage::Text(msg_str)).unwrap();

                session_data.sync_graph = Some(sync_graph);
            },
            Err(err) => client.send_message(
                &OwnedMessage::Text(format!("ERR {}", &err))).unwrap(),
        },
        // Connection closed
        OwnedMessage::Close(_) => {
            println!("{:?}", msg);
            return false;
        },
        // Ping
        OwnedMessage::Ping(_) => {
            println!("---ping---");
        },
        // Command
        _ => {
            if session_data.sync_graph.is_none() {
                println!(
                    "Error: Model not load but received {:?}", msg);
            } else {
                match get_response(msg, session_data) {
                    Ok(msg) => client.send_message(&msg).unwrap(),
                    Err(err) => println!("{}", err),
                }
            }
        },
    }
    true
}

fn main() {
    let mut server = Server::bind("127.0.0.1:5678").unwrap();
    loop {
        let connection = server.accept().unwrap();
        let mut client = connection.accept().unwrap();

        let ip = client.peer_addr().unwrap();
        println!("Connection from {}", ip);

        let mut session_data = SessionData::new();

        loop {
            if !session_loop(&mut client, &mut session_data) {
                break;
            }
        }
    }
}
