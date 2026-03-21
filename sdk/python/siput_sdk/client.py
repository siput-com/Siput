import requests


class SiputClient:
    def __init__(self, endpoint: str = 'http://localhost:8080'):
        self.endpoint = endpoint.rstrip('/')

    def _url(self, path: str) -> str:
        return f"{self.endpoint}{path}"

    def get_status(self) -> dict:
        r = requests.get(self._url('/status'))
        r.raise_for_status()
        return r.json()

    def get_balance(self, address: str) -> dict:
        r = requests.get(self._url(f'/balance/{address}'))
        r.raise_for_status()
        return r.json()

    def send_transaction(self, tx_data: dict) -> dict:
        r = requests.post(self._url('/transaction/send'), json=tx_data)
        r.raise_for_status()
        return r.json()
