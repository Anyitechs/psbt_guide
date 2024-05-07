use base64;
use dotenvy::dotenv;
use reqwest::blocking::Client as ReqClient;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
struct UnspentTxOutputs {
    txid: String,
    vout: u32,
    address: String,
    label: String,
    scriptPubKey: String,
    amount: f32,
    confirmations: u32,
    spendable: bool,
    solvable: bool,
    desc: String,
    parent_descs: Vec<String>,
    safe: bool,
}

#[derive(Debug, Clone)]
struct Input {
    txid: String,
    vout: u32,
}

#[derive(Debug, Deserialize)]
struct Psbt {
    psbt: String,
    fee: f32,
    changepos: i32,
}

#[derive(Debug, Deserialize)]
struct WalletProcessPsbt {
    psbt: String,
    complete: bool,
}

#[derive(Debug, Deserialize)]
struct FinalizedPsbtResponse {
    hex: String,
    complete: bool,
}

/// This function makes an RPC call to Bitcoin Core
fn send_rpc_request(method: &str, params: &Value, wallet_request: bool) -> Result<Value, Box<dyn Error>> {
    let rpc_url;
    let client = ReqClient::new();

    match wallet_request {
        true => {
            let rpc_link = env::var("RPC_HOST").expect("RPC_HOST not found in environment");
            rpc_url = format!("{rpc_link}/wallet/codeplanet");
        },
        false => {
            rpc_url = env::var("RPC_HOST").expect("RPC_HOST not found in environment");
        },
    }

    let rpc_user = env::var("RPC_USER").expect("RPC_USER not found in environment");
    let rpc_password = env::var("RPC_PASSWORD").expect("RPC_PASSWORD not found in environment");

    let credentials = format!("{}:{}", rpc_user, rpc_password);
    let encoded_credentials = format!("Basic {}", base64::encode(credentials));
    let auth = reqwest::header::HeaderValue::from_str(&encoded_credentials.as_str()).unwrap();

    let request_body = json!({
        "jsonrpc": "1.0",
        "id": "curltest",
        "method": method,
        "params": params,
    });


    let response = client
        .post(rpc_url)
        .header(reqwest::header::CONTENT_TYPE, "text/plain")
        .header(reqwest::header::AUTHORIZATION, auth)
        .body(request_body.to_string())
        .send()?;

    let json_response = response.json()?;

    Ok(json_response)
}

/// This function is used to deserialize the result value response
/// from Bitcoin Core.
fn deserialize_response<T: DeserializeOwned>(response: &Value) -> Option<T> {
    let json_response = &response["result"];
    let deserialized: Option<T> = serde_json::from_value(json_response.to_owned()).ok();
    deserialized
}

/// Creates a PSBT and returns the newly created PSBT.
fn create_psbt(input: Input, output: Vec<Value>) -> Result<Psbt, Box<dyn Error>> {
    let utxos = vec![json!({
        "txid": input.txid,
        "vout": input.vout,
    })];

    let body = json!([utxos, output]);

    let response = send_rpc_request("walletcreatefundedpsbt", &body, true);


    match response {
        Ok(psbt) => {
            let response_json: Psbt = deserialize_response(&psbt).unwrap();
            Ok(response_json)
        },
        Err(e) => {
            Err(e)
        }
    }
}

/// Joins multiple PSBTs into a single large PSBT.
fn join_psbt() -> Result<String, Box<dyn Error>> {
    let ifeanyi_wallet_psbt = env::var("IFEANYI_WALLET_PSBT").expect("User PSBT not found in environment");
    let codeplanet_wallet_psbt = env::var("CODEPLANET_WALLET_PSBT").expect("User PSBT not found in environment");

    let psbts = json!([ifeanyi_wallet_psbt, codeplanet_wallet_psbt]);

    let body = json!([psbts]);

    let response = send_rpc_request("joinpsbts", &body, false);

    match response {
        Ok(psbt) => {
            let result: String = deserialize_response(&psbt).unwrap();
            Ok(result)
        },
        Err(e) => {
            Err(e)
        }
    }
}

/// This function is used to sign the Joined PSBT.
fn wallet_process_psbt(psbt: String) -> Result<WalletProcessPsbt, Box<dyn Error>> {
    let body = json!([psbt]);

    let response = send_rpc_request("walletprocesspsbt", &body, true);

    match response {
        Ok(psbt) => {
            let result: WalletProcessPsbt = deserialize_response(&psbt).unwrap();
            Ok(result)
        },
        Err(e) => {
            Err(e)
        }
    }
}

/// Combines all signatures and input information into the same PSBT
fn combine_psbt(psbt: String) -> Result<String, Box<dyn Error>> {
    let body = json!([psbt]);

    let request_body = json!([body]);

    let response = send_rpc_request("combinepsbt", &request_body, false);

    match response {
        Ok(psbt) => {
            let result: String = deserialize_response(&psbt).unwrap();
            Ok(result)
        },
        Err(e) => {
            Err(e)
        }
    }
}

/// Finalizes the PSBT and creates a raw network transaction ready to be broadcasted.
fn finalize_psbt(psbt: String) -> Result<FinalizedPsbtResponse, Box<dyn Error>> {
    let body = json!([psbt]);

    let response = send_rpc_request("finalizepsbt", &body, false);

    match response {
        Ok(psbt) => {
            let result: FinalizedPsbtResponse = deserialize_response(&psbt).unwrap();
            Ok(result)
        },
        Err(e) => {
            Err(e)
        }
    }
}

/// Broadcasts the transaction to the network.
fn broadcast_transaction(hex: String) -> Result<String, Box<dyn Error>> {
    let body = json!([hex]);

    let response = send_rpc_request("sendrawtransaction", &body, false);

    match response {
        Ok(txid) => {
            let result: String = deserialize_response(&txid).unwrap();
            Ok(result)
        },
        Err(e) => {
            Err(e)
        }
    }
}

fn main() {
    dotenv().ok();

    let response = send_rpc_request("listunspent", &json!([]), true);

    match response {
        Ok(unspent_tx_outputs) => {
            let utxos: Option<Vec<UnspentTxOutputs>> =
                deserialize_response(&unspent_tx_outputs).unwrap();

            for (index, utxo) in utxos.unwrap().iter().enumerate() {
                
                // Manually selecting the utxo to spend
                if index == 1 {

                    let input = Input {
                        txid: utxo.txid.clone(),
                        vout: utxo.vout.clone(),
                    };
                    
                    let output = vec![json!({
                        "bcrt1qpfk7t93jfl240a4qv78kplqvqntxafg03rx68p": 0.0001
                    })];

                    let create_psbt = create_psbt(input, output);

                    match create_psbt {
                        Ok(val) => println!("val here: {:?}", val),
                        Err(e) => println!("error here: {:?}", e),
                    }
                }
            }
        }
        Err(e) => println!("Error here: {:?}", e),
    }
}
