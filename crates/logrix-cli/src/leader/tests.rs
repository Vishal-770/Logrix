use super::KubernetesLeaseElector;
use chrono::{Duration as ChronoDuration, Utc};
use logrix_core::ports::LeaderElectionPort;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

async fn start_fixed_server(status: u16, body: &str, requests: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    let body = body.to_string();
    tokio::spawn(async move {
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().await.expect("accept test request");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).await;
            let reason = if status == 200 { "OK" } else { "Error" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write test response");
        }
    });
    format!("http://{address}")
}

fn lease_json(resource_version: &str) -> String {
    lease_json_with_holder(resource_version, "pod-1", "2026-10-18T00:00:00Z")
}

fn lease_json_with_holder(resource_version: &str, holder: &str, renew_time: &str) -> String {
    format!(
        r#"{{"apiVersion":"coordination.k8s.io/v1","kind":"Lease","metadata":{{"name":"lease","namespace":"default","resourceVersion":"{resource_version}"}},"spec":{{"holderIdentity":"{holder}","leaseDurationSeconds":15,"acquireTime":"{renew_time}","renewTime":"{renew_time}"}}}}"#
    )
}

fn unexpired_other_holder_json(resource_version: &str) -> String {
    let renew_time = (Utc::now() + ChronoDuration::seconds(60)).to_rfc3339();
    lease_json_with_holder(resource_version, "pod-2", &renew_time)
}

#[tokio::test]
async fn test_kubernetes_lease_fail_closed_for_http_errors() {
    for (status, requests) in [(401, 1), (403, 1), (404, 2), (409, 1), (500, 1)] {
        let url = start_fixed_server(status, "{}", requests).await;
        let elector = KubernetesLeaseElector::new_with_timeout(
            "lease",
            "default",
            "pod-1",
            Some(url),
            Some("test-token".to_string()),
            std::time::Duration::from_secs(1),
        );
        assert!(!elector.try_acquire_or_renew().await.expect("fail closed"));
        assert!(!elector.is_leader().await);
    }
}

#[tokio::test]
async fn test_kubernetes_lease_fail_closed_for_parse_errors() {
    let url = start_fixed_server(200, "not-json", 1).await;
    let elector = KubernetesLeaseElector::new_with_timeout(
        "lease",
        "default",
        "pod-1",
        Some(url),
        None,
        std::time::Duration::from_secs(1),
    );
    assert!(!elector.try_acquire_or_renew().await.expect("fail closed"));
    assert!(!elector.is_leader().await);
}

#[tokio::test]
async fn test_kubernetes_lease_renews_with_resource_version() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut get_stream, _) = listener.accept().await.expect("accept GET");
        let mut get_request = [0_u8; 4096];
        let _ = get_stream.read(&mut get_request).await;
        write_json(&mut get_stream, &lease_json("7")).await;

        let (mut put_stream, _) = listener.accept().await.expect("accept PUT");
        let mut put_request = [0_u8; 8192];
        let size = put_stream.read(&mut put_request).await.expect("read PUT");
        let request = String::from_utf8_lossy(&put_request[..size]).to_string();
        let _ = request_tx.send(request);
        write_json(&mut put_stream, &lease_json("8")).await;
    });

    let elector = KubernetesLeaseElector::new_with_timeout(
        "lease",
        "default",
        "pod-1",
        Some(format!("http://{address}")),
        None,
        std::time::Duration::from_secs(1),
    );
    assert!(elector
        .try_acquire_or_renew()
        .await
        .expect("renew succeeds"));
    let request = request_rx.await.expect("capture PUT request");
    assert!(request.starts_with("PUT "));
    assert!(request.contains(r#""resourceVersion":"7""#));
}

#[tokio::test]
async fn test_kubernetes_lease_clears_cached_leader_for_unexpired_other_holder() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    tokio::spawn(async move {
        let (mut get_stream, _) = listener.accept().await.expect("accept initial GET");
        let mut request = [0_u8; 4096];
        let _ = get_stream.read(&mut request).await;
        write_json(&mut get_stream, &lease_json("7")).await;

        let (mut put_stream, _) = listener.accept().await.expect("accept renewal PUT");
        let _ = put_stream.read(&mut request).await;
        write_json(&mut put_stream, &lease_json("8")).await;

        let (mut get_stream, _) = listener.accept().await.expect("accept competing GET");
        let _ = get_stream.read(&mut request).await;
        write_json(&mut get_stream, &unexpired_other_holder_json("9")).await;
    });

    let elector = KubernetesLeaseElector::new_with_timeout(
        "lease",
        "default",
        "pod-1",
        Some(format!("http://{address}")),
        None,
        std::time::Duration::from_secs(1),
    );
    assert!(elector
        .try_acquire_or_renew()
        .await
        .expect("renew succeeds"));
    assert!(elector.is_leader().await);
    assert!(!elector
        .try_acquire_or_renew()
        .await
        .expect("standby succeeds"));
    assert!(!elector.is_leader().await);
}

async fn write_json(stream: &mut tokio::net::TcpStream, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("write JSON response");
}
