use std::io::{self, Write};
use txsignx_node::NodeContextReport;
pub fn write_report(out: &mut impl Write, node: &NodeContextReport) -> io::Result<()> {
    writeln!(
        out,
        "\nNode-reported context\n  Configured network: {}\n  Node network: {}\n  Chain tip: {} / {}",
        node.configured_network(),
        node.node_network(),
        node.tip().height,
        node.tip().hash
    )?;
    for i in node.inputs() {
        writeln!(
            out,
            "  Input [{}]: {:?}\n    confirmations: {:?}; coinbase: {:?}\n    PSBT prevout: {:?}",
            i.index, i.availability, i.confirmations, i.coinbase, i.prevout_verification
        )?;
    }
    writeln!(
        out,
        "  Mempool observations are point-in-time and not an atomic snapshot."
    )
}
