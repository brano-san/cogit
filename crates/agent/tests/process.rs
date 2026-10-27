use agent::Client;
use serde_json::json;
use std::process::{Command, Stdio};

#[test]
fn the_agent_process_answers_and_leaves_when_its_stdin_closes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cogit-agent"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let client =
        Client::connect(child.stdout.take().unwrap(), child.stdin.take().unwrap()).unwrap();
    assert_eq!(client.version(), env!("CARGO_PKG_VERSION"));
    assert!(client.call(json!("nothing"), None).unwrap().is_err());
    drop(client);
    assert!(child.wait().unwrap().success());
}
