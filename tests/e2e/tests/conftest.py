"""pytest-lsp fixtures for jinja-lsp e2e tests.

REQ-ARCH-04, REQ-ARCH-05, REQ-ARCH-06, REQ-ARCH-08 are verified here.
"""
import subprocess
import sys
from pathlib import Path

import pytest
import pytest_lsp
from lsprotocol import types as lsp

BINARY = Path(__file__).parent.parent.parent.parent / "target" / "debug" / "jinja-lsp"
FIXTURES = Path(__file__).parent.parent.parent / "fixtures"


@pytest_lsp.fixture(
    config=pytest_lsp.ClientServerConfig(
        server_command=[str(BINARY), "lsp"],
    )
)
async def client(lsp_client: pytest_lsp.LanguageClient):
    """Start the jinja-lsp server and initialize it."""
    params = lsp.InitializeParams(
        capabilities=lsp.ClientCapabilities(),
    )
    await lsp_client.initialize_session(params)
    yield lsp_client
    await lsp_client.shutdown_session()
