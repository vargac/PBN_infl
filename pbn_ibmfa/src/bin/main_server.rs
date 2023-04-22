// TODO make the "computing" state interruptible (alebo lahsie, len vypisat
// alertom po F5 pocas computing, ze by si restartovail server)
// TODO stlpec robustnost
// TODO leaf node aby sa to zarovnalo
// TODO ostatne nody vypisat ako target(input1, input2, ...)
// TODO monospace v leaf nodach aj state stlpci
// TODO ukazat Petovi Gambosovi ci sa s tym da nieco spravit?
//
// TODO benchmarking!!!
//
// TODO prerusenie vypoctu len ak fakt budem mat veeela casu
extern crate websocket;

use std::collections::HashMap;

use pbn_ibmfa::utils::add_self_regulations;
use pbn_ibmfa::symbolic_sync_graph::SymbSyncGraph;
use pbn_ibmfa::decision_tree::{DecisionTree, decision_tree};

use biodivine_lib_param_bn::{BooleanNetwork,
    symbolic_async_graph::{GraphColoredVertices, GraphColors, SymbolicContext}};

use websocket::{sync::{Server, Client, Stream}, OwnedMessage};


const ITERATIONS: usize = 10;

struct SessionData {
    sync_graph: Option<SymbSyncGraph>,
    attrs: Option<Vec<GraphColoredVertices>>,
    dtree_cache: HashMap<usize, DecisionTree>,
}

impl SessionData {
    fn new() -> Self {
        SessionData {
            sync_graph: None,
            attrs: None,
            dtree_cache: HashMap::new(),
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

fn tree_to_msg(
    tree: &DecisionTree,
    colors: &GraphColors,
    context: &SymbolicContext
) -> OwnedMessage {
    let mut buffer = String::new();
    tree_to_str_rec(tree, colors, context, &mut buffer);
    OwnedMessage::Text(buffer)
}

fn tree_to_str_rec(
    tree: &DecisionTree,
    colors: &GraphColors,
    context: &SymbolicContext,
    out: &mut String
) {
    let bdd_var_set = context.bdd_variable_set();
    match tree {
        DecisionTree::Leaf(driver_set) => {
            out.push('[');
            for (var_id, val) in driver_set {
                out.push(' ');
                out.push_str(&bdd_var_set.name_of(
                        context.get_state_variable(*var_id)));
                out.push('=');
                out.push(if *val { '1' } else { '0' });
            }
            out.push_str(" ]");
        },
        DecisionTree::Node(node) => {
            let fix_var = node.get_fix();
            let fix_false = colors.copy(
                colors.as_bdd().var_select(fix_var, false));
            let fix_true = colors.copy(
                colors.as_bdd().var_select(fix_var, true));

            out.push_str(&bdd_var_set.name_of(fix_var));
            out.push(' ');
            out.push_str(&fix_false.exact_cardinality().to_str_radix(10));
            out.push(' ');
            out.push_str(&fix_true.exact_cardinality().to_str_radix(10));
            out.push(' ');
            tree_to_str_rec(&node.get_childs()[0], &fix_false, context, out);
            out.push(' ');
            tree_to_str_rec(&node.get_childs()[1], &fix_true, context, out);
        }
    }
}

fn get_response(msg: OwnedMessage, session_data: &mut SessionData)
-> Result<OwnedMessage, String> {
    let sync_graph = session_data.sync_graph.as_ref().unwrap();
    match msg {
        OwnedMessage::Text(msg) => {
            if msg == "START" {
                let mut attrs = sync_graph.fixed_point_attractors();
                attrs.sort_by(|a1, a2| a2.colors().exact_cardinality()
                    .cmp(&a1.colors().exact_cardinality())); // descending
                let msg = attrs_to_msg(&attrs, sync_graph.symbolic_context());
                session_data.attrs = Some(attrs);
                Ok(msg)
            } else if msg.starts_with("TREE ") {
                match &session_data.attrs {
                    None => Err(format!("Error: '{msg:?}' before attractors")),
                    Some(attrs) => {
                        /*
                        Ok(OwnedMessage::Text(String::from(
                            "DN1 [ v_her6=0 v_elavl3_HuC=0 ] DN2 [ v_miR_9=0 ] \
                             [ v_Progenitor=1 v_her6=0 ]")))
                        */
                        let id = msg.rsplit(' ').next().unwrap()
                            .parse::<usize>().unwrap();
                        let dtree = session_data.dtree_cache
                            .entry(id)
                            .or_insert_with(|| {
                                let attr = &attrs[id];
                                decision_tree(&sync_graph, ITERATIONS,
                                    (&attr.vertices(), &attr.colors()))
                            });
                        Ok(tree_to_msg(&dtree, &attrs[id].colors(),
                                       sync_graph.symbolic_context()))
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
        OwnedMessage::Ping(data) => {
            println!("---ping---");
            client.send_message(&OwnedMessage::Pong(data)).unwrap();
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