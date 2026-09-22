#!/usr/bin/env python3
"""Explicit M6 local-node API verification. No signing, finalization or broadcast."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run():
    util = load('api_verify', 'verify-api.py')
    fixtures = load('m5_fixtures', 'verify-regtest.py')
    bitcoind = os.environ.get('BITCOIND', '/opt/homebrew/bin/bitcoind')
    cli = os.environ.get('BITCOIN_CLI', '/opt/homebrew/bin/bitcoin-cli')
    for executable in [bitcoind, cli, util.API]:
        assert Path(executable).is_file() and os.access(executable, os.X_OK)
    datadir = Path(tempfile.mkdtemp(prefix='txsignx-m6-regtest-')).resolve()
    assert datadir != (Path.home() / '.bitcoin').resolve()
    rpc_port, p2p_port = fixtures.reserve_ports()
    api_port = util.port()
    assert api_port not in [rpc_port, p2p_port]
    core_process = api_process = None
    core_args = ['-regtest', '-datadir=' + str(datadir), '-rpcconnect=127.0.0.1', '-rpcport=' + str(rpc_port)]

    def core(method, *args, wallet=False):
        argv = [cli, *core_args]
        if wallet:
            argv += ['-rpcwallet=txsignx-m6']
        result = subprocess.run(argv + [method] + [a if isinstance(a, str) else json.dumps(a) for a in args],
                                capture_output=True, text=True, timeout=30)
        if result.returncode:
            raise RuntimeError('Task Regtest RPC failed: ' + method)
        try:
            return json.loads(result.stdout)
        except json.JSONDecodeError:
            return result.stdout.strip()

    try:
        args = [bitcoind, '-regtest', '-datadir=' + str(datadir), '-server=1', '-daemon=0',
                '-connect=0', '-dnsseed=0', '-listen=0', '-networkactive=0', '-discover=0',
                '-rpcbind=127.0.0.1', '-rpcallowip=127.0.0.1', '-rpcport=' + str(rpc_port),
                '-port=' + str(p2p_port), '-dbcache=16', '-maxmempool=5', '-persistmempool=0',
                '-debuglogfile=0', '-printtoconsole=0']
        print('Temporary M6 datadir:', datadir, flush=True)
        print('-regtest present; datadir is NOT ~/.bitcoin; no public-chain synchronization will occur.', flush=True)
        print(f'Dedicated loopback RPC {rpc_port}; disabled P2P {p2p_port}; API {api_port}.', flush=True)
        version = subprocess.run([bitcoind, '-regtest', '-datadir=' + str(datadir), '--version'],
                                 capture_output=True, text=True, check=True).stdout.splitlines()[0]
        print(version, flush=True)
        core_process = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        ready = False
        for _ in range(100):
            assert core_process.poll() is None
            try:
                ready = core('getblockchaininfo')['chain'] == 'regtest'
                if ready:
                    break
            except RuntimeError:
                pass
            time.sleep(0.1)
        assert ready and core('getnetworkinfo')['networkactive'] is False and core('getpeerinfo') == []
        core('createwallet', 'txsignx-m6')
        mine = core('getnewaddress', '', 'bech32', wallet=True)
        pay = core('getnewaddress', '', 'bech32', wallet=True)
        change = core('getrawchangeaddress', 'bech32', wallet=True)
        descriptors = core('listdescriptors', False, wallet=True)['descriptors']
        wallet = {'network': 'regtest', 'derivation_window': 10, 'expected_change_outputs': [1]}
        for internal, key in [(False, 'external_descriptor'), (True, 'internal_descriptor')]:
            wallet[key] = next(d['desc'] for d in descriptors if d['active'] and d['internal'] == internal and d['desc'].startswith('wpkh('))
        core('generatetoaddress', 101, mine)
        coin = core('listunspent', 100, wallet=True)[0]
        # Test fixture only: Core-generated coinbase value is exactly 50 BTC here.
        assert coin['amount'] == 50
        script = bytes.fromhex(coin['scriptPubKey'])
        psbt = core('createpsbt', [{'txid': coin['txid'], 'vout': coin['vout']}], [{pay: 1}, {change: 48.99999}])
        psbt = fixtures.edit_input(psbt, b'\x01', struct.pack('<Q', 5_000_000_000) + fixtures.compact(len(script)) + script)
        base = f'http://127.0.0.1:{api_port}'
        api_process = subprocess.Popen([util.API, '--bind', f'127.0.0.1:{api_port}', '--network', 'regtest',
                                       '--rpc-url', f'http://127.0.0.1:{rpc_port}', '--rpc-cookie-file', str(datadir / 'regtest/.cookie')],
                                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        util.wait_api(base, api_process)
        assert util.http(base, 'capabilities')[1]['node_context_available'] is True
        data = {'psbt': psbt, 'wallet': wallet, 'node': {'use_configured_node': True}}
        status, report, _ = util.http(base, 'psbt/preflight', data)
        assert status == 200 and report['policy']['decision'] == 'pass'
        facts = report['node_context']['inputs'][0]
        assert facts['availability'] == 'confirmed_unspent' and facts['prevout_verification'] == 'match'
        assert facts['confirmations'] >= 100
        serialized = json.dumps(report)
        assert str(datadir) not in serialized
        assert (datadir / 'regtest/.cookie').read_text().strip() not in serialized
        assert wallet['external_descriptor'] not in serialized and wallet['internal_descriptor'] not in serialized
        print('Real API → Rust wallet/node policy PASS; privacy and mature prevout facts PASS.', flush=True)
        bad = fixtures.edit_input(psbt, b'\x01', struct.pack('<Q', 5_000_000_001) + fixtures.compact(len(script)) + script)
        data['psbt'] = bad
        status, report, _ = util.http(base, 'psbt/preflight', data)
        assert status == 200 and report['policy']['decision'] == 'block'
        assert any(f['code'] == 'TG016' for f in report['policy']['findings'])
        assert core('getrawmempool') == []
        print('Real API node prevout mismatch TG016 BLOCK; no broadcast occurred.', flush=True)
        core('stop')
        core_process.wait(timeout=30)
        assert util.http(base, 'psbt/preflight', data)[0] == 503
        print('Configured node unavailable fails closed with HTTP 503.', flush=True)
    finally:
        if api_process is not None:
            if api_process.poll() is None:
                api_process.terminate()
            api_process.wait(timeout=30)
        if core_process is not None and core_process.poll() is None:
            core('stop')
            core_process.wait(timeout=30)
        assert core_process is None or core_process.poll() is not None
        assert api_process is None or api_process.poll() is not None
        print('Task-owned API and Regtest processes stopped; exits verified.', flush=True)
        size = sum(p.stat().st_size for p in datadir.rglob('*') if p.is_file())
        print('Temporary M6 datadir logical size:', size, 'bytes', flush=True)
        assert datadir.name.startswith('txsignx-m6-regtest-') and datadir != (Path.home() / '.bitcoin').resolve()
        shutil.rmtree(datadir)
        assert not datadir.exists()
        print('Only exact task-created M6 datadir removed; absence verified.', flush=True)


if __name__ == '__main__':
    run()
