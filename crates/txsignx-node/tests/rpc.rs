use txsignx_node::*;
#[test]
fn cookie_files_are_required_and_errors_do_not_echo() {
    assert!(matches!(
        BitcoinCoreRpc::new(
            "http://127.0.0.1:28443",
            std::path::Path::new("/TXSIGNX_MISSING_COOKIE")
        ),
        Err(NodeError::InvalidCookie)
    ));
}
#[test]
fn node_chain_mapping_is_explicit() {
    for (s, n) in [
        ("main", bitcoin::Network::Bitcoin),
        ("test", bitcoin::Network::Testnet),
        ("testnet4", bitcoin::Network::Testnet4),
        ("signet", bitcoin::Network::Signet),
        ("regtest", bitcoin::Network::Regtest),
    ] {
        assert_eq!(node_network(s).unwrap(), n);
    }
    for s in ["bitcoin", "mainnet", "unknown", "REGTEST"] {
        assert_eq!(node_network(s), Err(NodeError::UnsupportedNetwork));
    }
}
