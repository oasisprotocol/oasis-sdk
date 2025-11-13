"""ROFL client for interacting with ROFL appd REST API.

Provides native python methods for the ROFL appd REST API endpoints.
"""

from __future__ import annotations

import asyncio
import logging
from typing import Any

from .async_rofl_client import AsyncRoflClient

logger = logging.getLogger(__name__)


class RoflClient:
    """Client for interacting with ROFL REST API.

    Provides methods for key fetching through the ROFL REST API.
    """

    def __init__(self, url: str = "") -> None:
        """Initialize ROFL client.

        :param url: Optional URL for HTTP transport (defaults to Unix socket)
        """
        self.client = AsyncRoflClient(url)

    def __getattr__(self, name: str) -> Any:
        """Dynamically create sync wrappers for async methods.

        :param name: Method name to look up
        :return: Sync wrapper function for the async method
        """
        attr = getattr(self.client, name)
        if asyncio.iscoroutinefunction(attr):

            def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
                return asyncio.run(attr(*args, **kwargs))

            return sync_wrapper
        return attr
