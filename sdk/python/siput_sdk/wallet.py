import secrets
import hashlib


class Wallet:
    @staticmethod
    def create() -> dict:
        entropy = secrets.token_bytes(32)
        address = hashlib.sha256(entropy).hexdigest()[:40]
        return {
            'private_key': entropy.hex(),
            'address': address,
        }

    @staticmethod
    def sign_transaction(private_key: str, tx_data: dict) -> dict:
        payload = f"{tx_data!r}:{private_key}".encode('utf-8')
        signature = hashlib.sha256(payload).hexdigest()
        return {**tx_data, 'signature': signature}
