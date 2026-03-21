# Siput Python SDK

## Install

pip install -e .

## Usage

from siput_sdk.client import SiputClient

client = SiputClient('http://127.0.0.1:8080')
print(client.get_status())
