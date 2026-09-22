#!/usr/bin/env python3
"""Isolated Regtest verification for capstone Transaction Explorer.

Never uses user's default ~/.bitcoin datadir. Network is completely disabled (networkactive=0).
"""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BITCOIND = os.environ.get('BITCOIND', '/opt/homebrew/bin/bitcoind')
BITCOIN_CLI = os.environ.get('BITCOIN_CLI', '/opt/homebrew/bin/bitcoin-cli')
TXSIGNX = os.environ.get('TXSIGNX', str(ROOT / 'target/debug/txsignx'))


def reserve_ports():
    sockets = []
    try:
        for _ in range(2):
            s = socket.socket()
            s.bind(('127.0.0.1', 0))
            sockets.append(s)
        ports = [s.getsockname()[1] for s in sockets]
        assert len(set(ports)) == 2
        return ports
    finally:
        for s in sockets:
            s.close()


def main():
    for executable in (BITCOIND, BITCOIN_CLI, TXSIGNX):
        if not Path(executable).is_file() or not os.access(executable, os.X_OK):
            raise RuntimeError(f'Required executable unavailable: {executable}')

    datadir = Path(tempfile.mkdtemp(prefix='txsignx-explorer-regtest-')).resolve()
    assert datadir != (Path.home() / '.bitcoin').resolve()

    process = None
    rpc_port, p2p_port = reserve_ports()
    core_args = [
        '-regtest',
        '-datadir=' + str(datadir),
        '-rpcconnect=127.0.0.1',
        '-rpcport=' + str(rpc_port),
    ]

    def core(method, *args, wallet=None):
        argv = [BITCOIN_CLI, *core_args]
        if wallet:
            argv.append(f'-rpcwallet={wallet}')
        argv += [
            method,
            *[
                a if isinstance(a, str) else json.dumps(a, separators=(',', ':'))
                for a in args
            ],
        ]
        p = subprocess.run(argv, capture_output=True, text=True, timeout=30)
        if p.returncode:
            raise RuntimeError(
                f'Bitcoin Core RPC failed: {method} ({p.stderr.strip()})'
            )
        try:
            return json.loads(p.stdout)
        except json.JSONDecodeError:
            return p.stdout.strip()

    try:
        argv = [
            BITCOIND,
            '-regtest',
            '-datadir=' + str(datadir),
            '-server=1',
            '-daemon=0',
            '-connect=0',
            '-dnsseed=0',
            '-listen=0',
            '-networkactive=0',
            '-discover=0',
            '-rpcbind=127.0.0.1',
            '-rpcallowip=127.0.0.1',
            '-rpcport=' + str(rpc_port),
            '-port=' + str(p2p_port),
            '-txindex=1',
            '-fallbackfee=0.0001',
            '-dbcache=16',
            '-maxmempool=5',
            '-persistmempool=0',
            '-debuglogfile=0',
            '-printtoconsole=0',
        ]
        print(f'Starting isolated Regtest daemon in {datadir}...')
        process = subprocess.Popen(
            argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )

        ready = False
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError('Regtest daemon exited unexpectedly')
            try:
                info = core('getblockchaininfo')
                if info['chain'] == 'regtest':
                    ready = True
                    break
            except RuntimeError:
                pass
            time.sleep(0.1)

        assert ready, 'Regtest daemon readiness timeout'
        assert core('getnetworkinfo')['networkactive'] is False
        print('Isolated Regtest daemon running with networkactive=0.')

        wallet_name = 'capstone-explorer-test'
        core('createwallet', wallet_name)

        mining_addr = core('getnewaddress', wallet=wallet_name)
        # Mine 101 blocks to mature coinbase rewards
        core('generatetoaddress', 101, mining_addr, wallet=wallet_name)

        # Transaction A: Confirmed transaction
        dest_addr_a = core('getnewaddress', wallet=wallet_name)
        txid_a = core('sendtoaddress', dest_addr_a, 5.0, wallet=wallet_name)
        # Mine 2 blocks to confirm txid_a
        core('generatetoaddress', 2, mining_addr, wallet=wallet_name)

        # Transaction B: Unconfirmed mempool transaction spending from confirmed outputs
        dest_addr_b = core('getnewaddress', wallet=wallet_name)
        txid_b = core('sendtoaddress', dest_addr_b, 1.5, wallet=wallet_name)

        # Inspect Transaction A with TxSignX
        cookie_path = str(datadir / 'regtest' / '.cookie')
        node_url = f'http://127.0.0.1:{rpc_port}'

        print(f'Inspecting Confirmed Transaction A: {txid_a}...')
        cmd_a = [
            TXSIGNX,
            'tx',
            'inspect',
            '--txid',
            txid_a,
            '--node-url',
            node_url,
            '--cookie-file',
            cookie_path,
            '--network',
            'regtest',
            '--json',
        ]
        res_a = subprocess.run(cmd_a, capture_output=True, text=True, check=True)
        report_a = json.loads(res_a.stdout)

        assert report_a['txid'] == txid_a
        assert report_a['chain_context']['status'] == 'confirmed'
        assert report_a['chain_context']['confirmations'] == 2
        assert report_a['chain_context']['block_hash'] is not None
        assert report_a['total_input_sats'] is not None
        assert report_a['total_output_sats'] is not None
        assert report_a['fee_sats'] is not None
        assert report_a['fee_sats'] == report_a['total_input_sats'] - report_a['total_output_sats']
        assert report_a['fee_rate'] is not None
        assert report_a['fee_rate']['sat_per_vb'] > 0
        assert len(report_a['inputs']) > 0
        assert report_a['inputs'][0]['resolved_prevout'] is not None
        assert report_a['inputs'][0]['resolved_prevout']['value_sats'] > 0
        assert len(report_a['outputs']) > 0
        assert report_a['outputs'][0]['address'] is not None
        print('  Confirmed Transaction A verification: PASS')

        print(f'Inspecting Unconfirmed Transaction B: {txid_b}...')
        cmd_b = [
            TXSIGNX,
            'tx',
            'inspect',
            '--txid',
            txid_b,
            '--node-url',
            node_url,
            '--cookie-file',
            cookie_path,
            '--network',
            'regtest',
            '--json',
        ]
        res_b = subprocess.run(cmd_b, capture_output=True, text=True, check=True)
        report_b = json.loads(res_b.stdout)

        assert report_b['txid'] == txid_b
        assert report_b['chain_context']['status'] == 'mempool'
        assert report_b['chain_context'].get('confirmations') in (None, 0)
        assert report_b['chain_context'].get('block_hash') is None
        assert report_b['total_input_sats'] is not None
        assert report_b['fee_sats'] is not None
        assert report_b['fee_rate'] is not None
        print('  Unconfirmed Transaction B verification: PASS')

        # Test human-readable output
        print('Testing human-readable terminal output...')
        cmd_human = [
            TXSIGNX,
            'tx',
            'inspect',
            '--txid',
            txid_a,
            '--node-url',
            node_url,
            '--cookie-file',
            cookie_path,
            '--network',
            'regtest',
        ]
        res_human = subprocess.run(
            cmd_human, capture_output=True, text=True, check=True
        )
        stdout = res_human.stdout
        assert 'TxSignX Transaction Analysis' in stdout
        assert 'Status: confirmed' in stdout
        assert 'Confirmations: 2' in stdout
        assert 'Fee rate:' in stdout
        assert 'Input total:' in stdout
        assert 'Output total:' in stdout
        assert 'Previous output value:' in stdout
        print('  Human-readable terminal output: PASS')

        print('\n=== ALL CAPSTONE REGTEST EXPLORER CHECKS PASSED ===\n')

    finally:
        if process:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
        shutil.rmtree(datadir, ignore_errors=True)
        print(f'Cleaned up temporary datadir: {datadir}')


if __name__ == '__main__':
    main()
