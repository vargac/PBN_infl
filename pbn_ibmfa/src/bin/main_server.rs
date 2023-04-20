extern crate websocket;

use pbn_ibmfa::utils::add_self_regulations;
use pbn_ibmfa::symbolic_sync_graph::SymbSyncGraph;

use biodivine_lib_param_bn::{BooleanNetwork,
    symbolic_async_graph::{GraphColoredVertices, SymbolicContext}};

use websocket::{sync::{Server, Client, Stream}, OwnedMessage};


struct SessionData {
    sync_graph: Option<SymbSyncGraph>,
    attrs: Option<Vec<GraphColoredVertices>>,
}

impl SessionData {
    fn new() -> Self {
        SessionData {
            sync_graph: None,
            attrs: None,
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
            } else {
                Err(format!("Error: unexpected command '{:?}'", msg))
            }
        },
        _ => Err(format!("Error: unexpected message type '{:?}'", msg)),
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
