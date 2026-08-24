use hive_core::types::{AgentCapability, TaskSpec};
use hive_p2p::client::SwarmTcpClient;
use hive_p2p::protocol::SwarmMessage;
use hive_p2p::server::SwarmTcpNode;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_tcp_node_client_communication() {
    let bind_addr = "127.0.0.1:19283";
    let node = SwarmTcpNode::new(bind_addr, 100);
    let mut rx = node.tx.subscribe();

    node.start().await.expect("Failed to start TCP node");
    sleep(Duration::from_millis(50)).await;

    let task = TaskSpec::new(
        "Delegator_TCP_Test",
        "0011223344",
        AgentCapability::SmartContractAuditor,
        "TCP Socket Test Task",
        "contract Test {}",
        50,
    );

    let msg = SwarmMessage::TaskRfq(task.clone());
    let ack = SwarmTcpClient::send_message(bind_addr, &msg)
        .await
        .expect("Client send failed");

    assert!(ack.contains("ACK"));

    // Verify received by server channel
    let received_msg = rx.recv().await.expect("Channel receive failed");
    match received_msg {
        SwarmMessage::TaskRfq(received_task) => {
            assert_eq!(received_task.id, task.id);
            assert_eq!(received_task.delegator, "Delegator_TCP_Test");
        }
        _ => panic!("Expected TaskRfq message"),
    }
}
