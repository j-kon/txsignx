#!/usr/bin/env python3
"""Node-free M6 HTTP integration. Only starts and stops its own loopback API."""
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
API = os.environ.get('TXSIGNX_API', str(ROOT / 'target/debug/txsignx-api'))


def port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def http(base, path, data=None, origin='http://127.0.0.1:5173', raw=None, content_type='application/json'):
    body = raw if raw is not None else (json.dumps(data).encode() if data is not None else None)
    headers = {'Origin': origin}
    if body is not None:
        headers['Content-Type'] = content_type
    req = urllib.request.Request(base + '/api/v1/' + path, data=body, headers=headers)
    # Do not honor ambient proxies for task-owned local integration requests.
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        response = opener.open(req, timeout=35)
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
        assert process.poll() is None, 'Task API exited during startup'
        try:
            if http(base, 'health')[0] == 200:
                return
        except (OSError, urllib.error.URLError):
            pass
        time.sleep(0.1)
    raise RuntimeError('Task API readiness timeout')


def run():
    base = f'http://127.0.0.1:{port()}'
    process = subprocess.Popen([API, '--bind', base.removeprefix('http://')],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        wait_api(base, process)
        status, capabilities, _ = http(base, 'capabilities')
        assert status == 200 and not capabilities['node_context_available']
        assert not capabilities['broadcast_via_api'] and not capabilities['signing']
        status, catalog, _ = http(base, 'policies')
        assert status == 200 and len(catalog['active_rules']) == 15 and len(catalog['deferred_rules']) == 2
        assert http(base, 'policies/TG017')[1]['code'] == 'TG017'
        assert http(base, 'policies/TG999')[0] == 404
        for name in ['legacy', 'segwit']:
            text = (ROOT / f'crates/txsignx-core/tests/fixtures/{name}.hex').read_text().strip()
            status, report, _ = http(base, 'transactions/inspect', {'raw_transaction': text})
            assert status == 200 and report['txid'] == '15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6'
            assert report['fee_sats'] is None and 'network' not in report
        print('HTTP raw legacy / SegWit facts PASS', flush=True)
        for name, decision, code in [('pass', 'pass', None), ('unusual-sighash', 'review', 'TG011'), ('800k-fee', 'block', 'TG002')]:
            text = (ROOT / f'fixtures/policy/{name}.b64').read_text()
            assert http(base, 'psbt/inspect', {'psbt': text})[0] == 200
            status, report, _ = http(base, 'psbt/preflight', {'psbt': text})
            assert status == 200 and report['policy']['decision'] == decision
            assert 'node_context' not in report and 'wallet_context' not in report
            if code:
                assert any(f['code'] == code for f in report['policy']['findings'])
            skipped = {r['code'] for r in report['policy']['rule_evaluations'] if r['status'] == 'not_evaluated'}
            assert {'TG001', 'TG006', 'TG015', 'TG016', 'TG017'} <= skipped
            print('HTTP actual Rust', decision.upper(), 'and skipped node coverage PASS', flush=True)
        wallet = {'network': 'regtest', 'external_descriptor': (ROOT / 'fixtures/wallet/external.desc').read_text().strip(),
                  'internal_descriptor': (ROOT / 'fixtures/wallet/internal.desc').read_text().strip(),
                  'derivation_window': 10, 'expected_change_outputs': [1]}
        data = {'psbt': (ROOT / 'fixtures/wallet/payment.b64').read_text(), 'wallet': wallet}
        status, report, _ = http(base, 'psbt/preflight', data)
        assert status == 200 and report['policy']['decision'] == 'pass'
        assert wallet['external_descriptor'] not in json.dumps(report)
        data['node'] = {'use_configured_node': True}
        assert http(base, 'psbt/preflight', data)[0] == 503
        for data, code in [({'psbt': 'PRIVATE_MARKER'}, 422), ({'psbt': 'A' * (1024 * 1024 + 1)}, 413),
                           ({'psbt': 'PRIVATE_MARKER', 'rpc_url': 'http://example.invalid'}, 400)]:
            status, error, _ = http(base, 'psbt/inspect', data)
            assert status == code and 'PRIVATE_MARKER' not in json.dumps(error)
        assert http(base, 'psbt/inspect', raw=b'{')[0] == 400
        assert http(base, 'psbt/inspect', raw=b'{}', content_type='text/plain')[0] == 415
        assert http(base, 'psbt/inspect', raw=b'A' * (2 * 1024 * 1024 + 1))[0] == 413
        status, _, headers = http(base, 'health', origin='http://evil.invalid')
        assert status == 403 and 'Access-Control-Allow-Origin' not in headers
        assert http(base, 'psbt/broadcast', {})[0] == 404
        print('HTTP wallet, no configured node, malformed/media/size, CORS and no-broadcast checks PASS', flush=True)
    finally:
        if process.poll() is None:
            process.terminate()
        process.wait(timeout=30)
        print('Task-owned API stopped and exit verified.', flush=True)
    try:
        http(base, 'health')
    except (OSError, urllib.error.URLError):
        print('API unavailable after shutdown confirmed.', flush=True)
    else:
        raise AssertionError('Stopped API still reachable')


if __name__ == '__main__':
    run()
