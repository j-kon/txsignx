use txsignx_node::{NodeError, RpcEndpoint};
#[test]
fn only_exact_loopback_endpoints_are_accepted() {
    for url in ["http://127.0.0.1:18443", "http://[::1]:28443"] {
        assert!(RpcEndpoint::parse(url).is_ok());
    }
    for url in [
        "http://localhost:1",
        "https://127.0.0.1:1",
        "http://127.1:1",
        "http://2130706433:1",
        "http://127.0.0.1:0",
        "http://127.0.0.1:01",
        "http://127.0.0.1:65536",
        "http://127.0.0.1:1/",
        "http://127.0.0.1:1?x",
        "http://127.0.0.1:1#x",
        "http://user:secret@127.0.0.1:1",
        "http://127.0.0.1:1\\evil",
        "http://[::ffff:127.0.0.1]:1",
        "http://10.0.0.1:1",
        "http://192.168.1.1:1",
        "http://8.8.8.8:1",
        "http://site.onion:1",
        "http://127.0.0.1:+1",
        "http://127.0.0.1:1\n",
    ] {
        assert_eq!(
            RpcEndpoint::parse(url).err(),
            Some(NodeError::InvalidEndpoint)
        );
    }
}
#[test]
fn config_errors_are_static_and_sanitized() {
    let e = RpcEndpoint::parse("http://user:SECRET_MARKER@remote:80")
        .err()
        .unwrap();
    assert!(!format!("{e:?} {e}").contains("SECRET_MARKER"));
}
