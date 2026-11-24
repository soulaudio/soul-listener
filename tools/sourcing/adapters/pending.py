"""Adapters that exist as a shape only, because no credentials exist yet.

These are deliberately NOT written as speculative implementations. Mouser's
Swagger specification is not retrievable unauthenticated (it 302s into a
geo-context setter), and transcribing field names from prose is exactly the guess
that produces a parser which looks right and silently maps nothing — the mistake
TME's own documentation already caused once in this repo.

So each one reports itself unavailable and says what it needs. The pipeline
treats an unavailable vendor as a normal partial run, which means adding a real
implementation later changes nothing else.

The sequence when a key arrives, per `docs/sourcing/README.md`: make one request
for `CS43131-CNZ` (already in the snapshot with hand-researched figures, so the
response can be checked against something), commit it redacted as a fixture,
write `ATTR_MAP` and the hydrate step against that fixture with a test, then run
the categories.
"""

from __future__ import annotations

from .base import Adapter, VendorPart


class _Unavailable(Adapter):
    needs: str = ""

    def available(self) -> bool:
        return False

    def lookup_mpns(self, mpns: list[str]) -> list[VendorPart]:
        raise NotImplementedError(
            f"{self.display_name}: {self.needs}. See docs/sourcing/DISTRIBUTOR-API-SETUP.md"
        )

    def search_category(
        self, category: str, limit: int, phrase: str | None = None
    ) -> list[VendorPart]:
        raise NotImplementedError(
            f"{self.display_name}: {self.needs}. See docs/sourcing/DISTRIBUTOR-API-SETUP.md"
        )


class MouserAdapter(_Unavailable):
    name = "mouser"
    display_name = "Mouser Electronics"
    warehouse = "DE"
    needs = (
        "MOUSER_SEARCH_API_KEY, plus one recorded response to write the field mapping "
        "against — the Swagger spec is not retrievable unauthenticated"
    )
