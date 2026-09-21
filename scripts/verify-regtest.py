#!/usr/bin/env python3
"""Explicit opt-in real-node M5 verification. Never uses a user's Bitcoin datadir."""
import base64
import json
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BITCOIND = os.environ.get('BITCOIND', '/opt/homebrew/bin/bitcoind')
BITCOIN_CLI = os.environ.get('BITCOIN_CLI', '/opt/homebrew/bin/bitcoin-cli')
TXSIGNX = os.environ.get('TXSIGNX', str(ROOT / 'target/debug/txsignx'))


def compact(n):
    if n < 253:
        return bytes([n])
    if n <= 65535:
        return b'\xfd' + struct.pack('<H', n)
    return b'\xfe' + struct.pack('<I', n)


def read_compact(data, pos):
    n = data[pos]
    pos += 1
    if n < 253:
        return n, pos
    size = {253: 2, 254: 4, 255: 8}[n]
    return int.from_bytes(data[pos:pos + size], 'little'), pos + size


def maps(text):
    data = base64.b64decode(text, validate=True)
    assert data[:5] == b'psbt\xff'
    pos, result = 5, []
    while pos < len(data):
        entries = []
        while True:
            size, pos = read_compact(data, pos)
            if size == 0:
                break
            key = data[pos:pos + size]
            pos += size
            size, pos = read_compact(data, pos)
            value = data[pos:pos + size]
            pos += size
            entries.append((key, value))
        result.append(entries)
    return result


def encode(groups):
    data = b'psbt\xff'
    for group in groups:
        for key, value in group:
            data += compact(len(key)) + key + compact(len(value)) + value
        data += b'\0'
    return base64.b64encode(data).decode()


def edit_input(text, key, value):
    groups = maps(text)
    groups[1] = [(k, v) for k, v in groups[1] if k != key]
    groups[1].append((key, value))
    return encode(groups)


def reserve_ports():
    sockets = []
    try:
        for _ in range(2):
            s = socket.socket()
            s.bind(('127.0.0.1', 0))
            sockets.append(s)
        ports = [s.getsockname()[1] for s in sockets]
        assert len(set(ports)) == 2
        assert not set(ports) & {8332, 8333, 18332, 18333, 18443, 18444, 38332, 38333, 48332, 48333}
        return ports
    finally:
        for s in sockets:
            s.close()


def main():
    for executable in (BITCOIND, BITCOIN_CLI, TXSIGNX):
        if not Path(executable).is_file() or not os.access(executable, os.X_OK):
            raise RuntimeError('Required explicit executable is unavailable')
    datadir = Path(tempfile.mkdtemp(prefix='txsignx-m5-regtest-')).resolve()
    assert datadir != (Path.home() / '.bitcoin').resolve()
    process = None
    rpc_port, p2p_port = reserve_ports()
    core_args = ['-regtest', '-datadir=' + str(datadir), '-rpcconnect=127.0.0.1', '-rpcport=' + str(rpc_port)]

    def core(method, *args, wallet=True):
        argv = [BITCOIN_CLI, *core_args]
        if wallet:
            argv.append('-rpcwallet=txsignx-m5')
        argv += [method] + [a if isinstance(a, str) else json.dumps(a, separators=(',', ':')) for a in args]
        p = subprocess.run(argv, capture_output=True, text=True, timeout=30)
        if p.returncode:
            raise RuntimeError('Temporary Regtest RPC failed: ' + method)
        try:
            return json.loads(p.stdout)
        except json.JSONDecodeError:
            return p.stdout.strip()

    node = ['--network', 'regtest', '--rpc-url', f'http://127.0.0.1:{rpc_port}', '--rpc-cookie-file', str(datadir / 'regtest/.cookie')]
    wallet_opts = ['--external-descriptor-file', str(datadir / 'external.desc'), '--internal-descriptor-file', str(datadir / 'internal.desc'), '--expected-change-output', '1']
    private_markers = []

    def txsignx(text, code=0, command='preflight', wallet=True, extra=(), mode='stdin', human=False):
        source = ['--stdin']
        if mode == 'positional':
            source = [text]
        elif mode == 'file':
            (datadir / 'candidate.psbt').write_text(text)
            source = ['--file', str(datadir / 'candidate.psbt')]
        argv = [TXSIGNX, 'psbt', command, *source, *node]
        if wallet:
            argv += wallet_opts
        argv += list(extra)
        if not human:
            argv.append('--json')
        p = subprocess.run(argv, input=text if mode == 'stdin' else None, capture_output=True, text=True, timeout=30)
        assert p.returncode == code, f'{command}: expected exit {code}, actual {p.returncode}; output suppressed'
        assert all(marker not in p.stdout + p.stderr for marker in private_markers)
        if code == 1:
            assert not p.stdout
            assert 'error:' in p.stderr
            return None
        assert not p.stderr
        return p.stdout if human else json.loads(p.stdout)

    def finding(r, code, decision):
        assert r['policy']['decision'] == decision
        assert any(f['code'] == code for f in r['policy']['findings'])
        print(code, decision.upper(), 'PASS', flush=True)

    def mempool():
        return set(core('getrawmempool', wallet=False))

    try:
        argv = [BITCOIND, '-regtest', '-datadir=' + str(datadir), '-server=1', '-daemon=0', '-connect=0', '-dnsseed=0', '-listen=0', '-networkactive=0', '-discover=0', '-rpcbind=127.0.0.1', '-rpcallowip=127.0.0.1', '-rpcport=' + str(rpc_port), '-port=' + str(p2p_port), '-dbcache=16', '-maxmempool=5', '-persistmempool=0', '-debuglogfile=0', '-printtoconsole=0']
        assert '-regtest' in argv and '-datadir=' + str(datadir) in argv
        print('Temporary datadir:', datadir, flush=True)
        print('-regtest present; datadir is NOT ~/.bitcoin; no public-chain synchronization will occur.', flush=True)
        print(f'Loopback RPC port {rpc_port}; dedicated disabled P2P port {p2p_port}; public networking disabled.', flush=True)
        # Version calls carry the same explicit Regtest/datadir safety arguments.
        version = subprocess.run([BITCOIND, '-regtest', '-datadir=' + str(datadir), '--version'], capture_output=True, text=True, check=True).stdout.splitlines()[0]
        print(version, flush=True)
        process = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        ready = False
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError('Task-created Regtest node exited before readiness')
            try:
                info = core('getblockchaininfo', wallet=False)
                if info['chain'] == 'regtest':
                    ready = True
                    break
            except RuntimeError:
                pass
            time.sleep(0.1)
        assert ready, 'Task-created Regtest startup timeout'
        assert core('getnetworkinfo', wallet=False)['networkactive'] is False
        assert not core('getpeerinfo', wallet=False)
        core('createwallet', 'txsignx-m5', wallet=False)
        mine = core('getnewaddress', '', 'bech32')
        pay = core('getnewaddress', '', 'bech32')
        change = core('getrawchangeaddress', 'bech32')
        descriptors = core('listdescriptors', False)['descriptors']
        for internal, name in [(False, 'external'), (True, 'internal')]:
            desc = next(d['desc'] for d in descriptors if d['active'] and d['internal'] == internal and d['desc'].startswith('wpkh('))
            (datadir / (name + '.desc')).write_text(desc)
            private_markers.append(desc)
            import re
            private_markers += re.findall(r'[xt]pub[1-9A-HJ-NP-Za-km-z]+', desc)
        # Used only for leak assertions; never printed or persisted outside this datadir.
        private_markers.append((datadir / 'regtest/.cookie').read_text().strip())
        core('generatetoaddress', 105, mine, wallet=False)
        funds = sorted(core('listunspent', 100), key=lambda u: (-u['confirmations'], u['txid'], u['vout']))
        assert len(funds) >= 5

        def make(utxo, fee=1000, payment=100000000):
            amount = round(utxo['amount'] * 100000000)
            text = core('createpsbt', [{'txid': utxo['txid'], 'vout': utxo['vout']}], [{pay: payment / 100000000}, {change: (amount - payment - fee) / 100000000}], wallet=False)
            script = bytes.fromhex(utxo['scriptPubKey'])
            return edit_input(text, b'\x01', struct.pack('<Q', amount) + compact(len(script)) + script)

        def finalized(text):
            signed = core('walletprocesspsbt', text, True, 'ALL', True, False)
            done = core('finalizepsbt', signed['psbt'], False, wallet=False)
            assert done['complete'], 'External Core finalization failed'
            return done['psbt']

        text = make(funds[0])
        r = txsignx(text)
        assert r['policy']['decision'] == 'pass'
        i = r['node_context']['inputs'][0]
        assert i['availability'] == 'confirmed_unspent' and i['prevout_verification'] == 'match'
        assert i['confirmations'] >= 100
        print('Normal wallet + node PASS', flush=True)
        for mode in ['positional', 'file', 'stdin']:
            for human in [False, True]:
                txsignx(text, mode=mode, human=human)
        print('All PSBT input modes / human / JSON privacy PASS', flush=True)
        node[1] = 'bitcoin'
        finding(txsignx(text, 3, wallet=False), 'TG001', 'block')
        node[1] = 'regtest'
        groups = maps(text)
        raw = bytearray(dict(groups[0])[b'\x00'])
        raw[5:37] = bytes([0x55]) * 32
        groups[0] = [(b'\x00', bytes(raw))]
        finding(txsignx(encode(groups), 3, wallet=False), 'TG015', 'block')
        witness = dict(maps(text)[1])[b'\x01']
        wrong_amount = struct.pack('<Q', int.from_bytes(witness[:8], 'little') + 1) + witness[8:]
        finding(txsignx(edit_input(text, b'\x01', wrong_amount), 3, wallet=False), 'TG016', 'block')
        wrong_script = witness[:8] + b'\x16\x00\x14' + bytes([0x44]) * 20
        finding(txsignx(edit_input(text, b'\x01', wrong_script), 3, wallet=False), 'TG016', 'block')
        latest = core('getblock', core('getbestblockhash', wallet=False), 2, wallet=False)['tx'][0]
        immature = {'txid': latest['txid'], 'vout': 0, 'amount': latest['vout'][0]['value'], 'scriptPubKey': latest['vout'][0]['scriptPubKey']['hex']}
        immature_text = make(immature)
        finding(txsignx(immature_text, 3), 'TG006', 'block')
        core('generatetoaddress', 98, mine, wallet=False)
        finding(txsignx(immature_text, 3), 'TG006', 'block')
        core('generatetoaddress', 1, mine, wallet=False)
        assert txsignx(immature_text)['node_context']['inputs'][0]['confirmations'] == 100
        core('generatetoaddress', 1, mine, wallet=False)
        assert txsignx(immature_text)['node_context']['inputs'][0]['confirmations'] == 101
        print('Real coinbase boundary 99 BLOCK / 100 PASS / 101 PASS', flush=True)
        a = finalized(make(funds[1]))
        extracted = core('finalizepsbt', a, True, wallet=False)
        core('sendrawtransaction', extracted['hex'], wallet=False)
        conflict = make(funds[1], payment=110000000)
        finding(txsignx(conflict, 2), 'TG017', 'review')
        before = mempool()
        finding(txsignx(conflict, 2, command='broadcast'), 'TG017', 'review')
        assert mempool() == before
        ready_psbt = finalized(make(funds[2]))
        before = mempool()
        finding(txsignx(ready_psbt, 3, command='broadcast', extra=['--max-absolute-fee-sats', '0']), 'TG002', 'block')
        assert mempool() == before
        wallet_opts[-1] = '0'
        finding(txsignx(ready_psbt, 2, command='broadcast'), 'TG005', 'review')
        wallet_opts[-1] = '1'
        assert mempool() == before
        txsignx(make(funds[2]), 1, command='broadcast')
        assert mempool() == before
        node[1] = 'testnet'
        finding(txsignx(ready_psbt, 3, command='broadcast'), 'TG001', 'block')
        node[1] = 'regtest'
        assert mempool() == before
        invalid_final = edit_input(make(funds[3]), b'\x08', b'\x01\x01\x00')
        txsignx(invalid_final, 1, command='broadcast')
        assert mempool() == before
        print('BLOCK / REVIEW / unfinished / non-Regtest / Core-rejection no-broadcast gates PASS', flush=True)
        r = txsignx(ready_psbt, command='broadcast')
        txid = r['broadcast']['txid']
        assert r['broadcast']['network'] == 'regtest' and r['policy']['decision'] == 'pass'
        assert txid in mempool()
        print('Broadcast accepted on Regtest; confirmed in node mempool:', txid, flush=True)
        original_url = node[3]
        node[3] = f'http://127.0.0.1:{p2p_port}'
        txsignx(text, 1, wallet=False)
        node[3] = original_url
        print('RPC unavailable fails closed PASS', flush=True)
        print('ALL REAL REGTEST SCENARIOS PASSED', flush=True)
    finally:
        if process is not None and process.poll() is None:
            core('stop', wallet=False)
            process.wait(timeout=30)
        assert process is None or process.poll() is not None, 'Own node has not stopped; refusing deletion'
        print('Task-created Regtest process stopped and exit verified.', flush=True)
        size = sum(p.stat().st_size for p in datadir.rglob('*') if p.is_file())
        print('Temporary datadir logical size:', size, 'bytes', flush=True)
        assert datadir.name.startswith('txsignx-m5-regtest-') and datadir != (Path.home() / '.bitcoin').resolve()
        shutil.rmtree(datadir)
        assert not datadir.exists()
        print('Only task-created temporary datadir removed; removal verified.', flush=True)


if __name__ == '__main__':
    main()
