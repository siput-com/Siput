import pytest
from siput_sdk.client import SiputClient
from siput_sdk.wallet import Wallet


def test_wallet_create_and_sign():
    wallet = Wallet.create()
    assert 'address' in wallet and 'private_key' in wallet

    tx = {'to': 'abc123', 'amount': 100}
    signed = Wallet.sign_transaction(wallet['private_key'], tx)
    assert signed['signature'] is not None


def test_client_url_build(monkeypatch):
    client = SiputClient('http://localhost:1234')

    class E:
        def __init__(self):
            self.url = None

    e = E()

    def fake_get(url):
        e.url = url
        class R:
            status_code = 200
            def raise_for_status(self):
                pass
            def json(self):
                return {'ok': True}
        return R()

    monkeypatch.setattr('requests.get', fake_get)
    status = client.get_status()

    assert status == {'ok': True}
    assert e.url == 'http://localhost:1234/status'
