#!/usr/bin/env python3
"""Isolated Regtest verification for HTTP API Transaction Explorer.

Validates that txsignx-api correctly interacts with a real local Bitcoin Core
daemon in regtest mode (networkactive=0, txindex=1) to inspect confirmed and
mempool transactions by txid, deriving addresses, resolving prevouts, and
calculating fees without any signing or broadcast capabilities.
"""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BITCOIND = os.environ.get('BITCOIND', '/opt/homebrew/bin/bitcoind')
BITCOIN_CLI = os.environ.get('BITCOIN_CLI', '/opt/homebrew/bin/bitcoin-cli')
API_BIN = os.environ.get('TXSIGNX_API', str(ROOT / 'target/debug/txsignx-api'))


def reserve_ports(count=2):
    sockets = []
    try:
        for _ in range(count):
            s = socket.socket()
            s.bind(('127.0.0.1', 0))
            sockets.append(s)
        ports = [s.getsockname()[1] for s in sockets]
        assert len(set(ports)) == count
        return ports
    finally:
        for s in sockets:
            s.close()


def http(base, path, data=None):
    body = json.dumps(data).encode() if data is not None else None
    headers = {'Origin': 'http://127.0.0.1:5173'}
    if body is not None:
        headers['Content-Type'] = 'application/json'
    req = urllib.request.Request(f'{base}/api/v1/{path}', data=body, headers=headers)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        response = opener.open(req, timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        payload = response.read(8 * 1024 * 1024 + 1)
        assert len(payload) <= 8 * 1024 * 1024
        assert response.headers.get('Cache-Control') == 'no-store'
        assert response.headers.get('X-Content-Type-Options') == 'nosniff'
        return response.status, json.loads(payload), response.headers


def wait_api(base, process):
    for _ in range(100):
        assert process.poll() is None, 'API process exited unexpectedly'
        try:
            if http(base, 'health')[0] == 200:
                return
        except (OSError, urllib.error.URLError):
            pass
        time.sleep(0.1)
    raise RuntimeError('API readiness timeout')


def main():
    for executable in (BITCOIND, BITCOIN_CLI, API_BIN):
        if not Path(executable).is_file() or not os.access(executable, os.X_OK):
            raise RuntimeError(f'Required executable unavailable: {executable}')

    datadir = Path(tempfile.mkdtemp(prefix='txsignx-api-explorer-regtest-')).resolve()
    assert datadir != (Path.home() / '.bitcoin').resolve()

    core_process = api_process = None
    rpc_port, p2p_port, api_port = reserve_ports(3)
    core_args = [
        '-regtest',
        f'-datadir={datadir}',
        '-rpcconnect=127.0.0.1',
        f'-rpcport={rpc_port}',
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
            f'-datadir={datadir}',
            '-server=1',
            '-daemon=0',
            '-connect=0',
            '-dnsseed=0',
            '-listen=0',
            '-networkactive=0',
            '-discover=0',
            '-rpcbind=127.0.0.1',
            '-rpcallowip=127.0.0.1',
            f'-rpcport={rpc_port}',
            f'-port={p2p_port}',
            '-txindex=1',
            '-fallbackfee=0.0001',
            '-dbcache=16',
            '-maxmempool=5',
            '-persistmempool=0',
            '-debuglogfile=0',
            '-printtoconsole=0',
        ]
        print(f'Starting isolated Regtest daemon in {datadir}...')
        core_process = subprocess.Popen(
            argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )

        ready = False
        for _ in range(100):
            if core_process.poll() is not None:
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
        print('Isolated Regtest daemon running with networkactive=0 and txindex=1.')

        wallet_name = 'api-explorer-test'
        core('createwallet', wallet_name)

        mining_addr = core('getnewaddress', wallet=wallet_name)
        # Mine 101 blocks to mature coinbase rewards
        core('generatetoaddress', 101, mining_addr, wallet=wallet_name)

        # Transaction A: Confirmed transaction
        dest_addr_a = core('getnewaddress', wallet=wallet_name)
        txid_a = core('sendtoaddress', dest_addr_a, 5.0, wallet=wallet_name)
        # Mine 2 blocks to confirm txid_a
        core('generatetoaddress', 2, mining_addr, wallet=wallet_name)

        # Transaction B: Unconfirmed mempool transaction
        dest_addr_b = core('getnewaddress', wallet=wallet_name)
        txid_b = core('sendtoaddress', dest_addr_b, 1.5, wallet=wallet_name)

        cookie_path = str(datadir / 'regtest' / '.cookie')
        base = f'http://127.0.0.1:{api_port}'

        print(f'Starting txsignx-api connected to Regtest on port {api_port}...')
        api_argv = [
            API_BIN,
            '--bind', f'127.0.0.1:{api_port}',
            '--network', 'regtest',
            '--rpc-url', f'http://127.0.0.1:{rpc_port}',
            '--rpc-cookie-file', cookie_path,
        ]
        api_process = subprocess.Popen(
            api_argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        wait_api(base, api_process)
        print('txsignx-api is ready.')

        # 1. Verify capabilities
        status, cap, _ = http(base, 'capabilities')
        assert status == 200
        assert cap['transaction_explorer'] is True
        assert cap['txid_inspection'] is True
        assert cap['transaction_address_rendering'] is True
        assert cap['node_context_available'] is True
        assert cap['signing'] is False
        assert cap['finalization'] is False
        assert cap['broadcast_via_api'] is False
        print('  API capabilities verification: PASS')

        # 2. Inspect Confirmed Transaction A by txid
        print(f'Inspecting Confirmed Transaction A: {txid_a}...')
        status_a, report_a, _ = http(base, 'transactions/inspect', {'txid': txid_a})
        assert status_a == 200, f'Expected 200, got {status_a}: {report_a}'
        assert report_a['txid'] == txid_a
        assert report_a['chain_context']['status'] == 'confirmed'
        assert report_a['chain_context']['confirmations'] == 2
        assert report_a['chain_context']['block_hash'] is not None
        assert report_a['chain_context']['network'] == 'regtest'
        assert report_a['total_input_sats'] is not None
        assert report_a['total_output_sats'] is not None
        assert report_a['fee_sats'] is not None
        assert report_a['fee_sats'] == report_a['total_input_sats'] - report_a['total_output_sats']
        assert report_a['fee_rate'] is not None
        assert report_a['fee_rate']['sat_per_vb'] > 0
        assert len(report_a['inputs']) > 0
        assert report_a['inputs'][0]['resolved_prevout'] is not None
        assert report_a['inputs'][0]['resolved_prevout']['value_sats'] > 0
        assert report_a['inputs'][0]['resolved_prevout']['address'] is not None
        assert len(report_a['outputs']) > 0
        assert report_a['outputs'][0]['address'] is not None
        assert report_a['outputs'][0]['script_pubkey_asm'] is not None
        print('  Confirmed Transaction A via API: PASS')

        # 3. Inspect Unconfirmed Mempool Transaction B by txid
        print(f'Inspecting Unconfirmed Mempool Transaction B: {txid_b}...')
        status_b, report_b, _ = http(base, 'transactions/inspect', {'txid': txid_b})
        assert status_b == 200, f'Expected 200, got {status_b}: {report_b}'
        assert report_b['txid'] == txid_b
        assert report_b['chain_context']['status'] == 'mempool'
        assert report_b['chain_context'].get('confirmations') in (None, 0)
        assert report_b['chain_context'].get('block_hash') is None
        assert report_b['chain_context']['network'] == 'regtest'
        assert report_b['total_input_sats'] is not None
        assert report_b['fee_sats'] is not None
        assert report_b['fee_rate'] is not None
        assert report_b['inputs'][0]['resolved_prevout'] is not None
        print('  Unconfirmed Mempool Transaction B via API: PASS')

        # 4. Verify Raw inspection with optional explicit network works via API
        raw_tx_hex = core('getrawtransaction', txid_a)
        status_raw, report_raw, _ = http(
            base,
            'transactions/inspect',
            {'raw_transaction': raw_tx_hex, 'network': 'regtest'},
        )
        assert status_raw == 200
        assert report_raw['txid'] == txid_a
        assert report_raw.get('chain_context') is None
        assert report_raw.get('fee_sats') is None
        assert report_raw.get('total_input_sats') is None
        assert report_raw.get('fee_rate') is None
        assert report_raw['outputs'][0]['address'] is not None
        print('  Raw transaction inspection with explicit network: PASS')

        # 5. Verify security boundaries: Cookie and secret files never leak in responses
        serialized = json.dumps([report_a, report_b, report_raw])
        assert str(datadir) not in serialized
        assert (datadir / 'regtest/.cookie').read_text().strip() not in serialized
        print('  Privacy and credential non-disclosure verification: PASS')

        # 6. Verify node unavailable fails closed with 503
        core('stop')
        core_process.wait(timeout=30)
        status_err, err_body, _ = http(base, 'transactions/inspect', {'txid': txid_a})
        assert status_err == 503
        assert err_body['error']['code'] == 'node_unavailable'
        print('  Node unavailable fails closed with HTTP 503: PASS')

    finally:
        if api_process is not None and api_process.poll() is None:
            api_process.terminate()
            api_process.wait(timeout=10)
        if core_process is not None and core_process.poll() is None:
            try:
                core('stop')
            except Exception:
                core_process.kill()
            core_process.wait(timeout=10)

        assert api_process is None or api_process.poll() is not None
        assert core_process is None or core_process.poll() is not None
        print('All task-owned processes terminated.')

        if datadir.exists():
            shutil.rmtree(datadir)
            assert not datadir.exists()
            print(f'Cleaned up isolated temporary datadir: {datadir}')


if __name__ == '__main__':
    main()
