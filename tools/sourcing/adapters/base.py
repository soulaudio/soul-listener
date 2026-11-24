"""The vendor adapter seam.

Every distributor differs in how you authenticate, how you page, how you batch,
and what its parameters are called. None of them differ in what the pipeline
needs from them. So an adapter's job is exactly two things:

1. **Transport.** Authenticate, page, batch, retry — whatever that vendor needs.
2. **Name mapping.** Translate the vendor's parameter names onto the spec's
   attribute vocabulary (`"Supply voltage"` -> `supply_mv`).

An adapter must NOT interpret a value. It hands over `"1.65...5.5V DC"` exactly
as received. Everything about what that string means — units, SI prefixes,
ranges, which bound a constraint wants — happens in
`dap_hw_spec::parametric::derive`, which is `no_std`, dependency-free, and
covered by the same test run as the constraints that consume it.

That boundary is not stylistic. A unit misread by 1000x fails in two directions
and only one of them is visible: too strict rejects every part in the category
and somebody notices, too lax puts a 5 V part on a 3.3 V rail and nothing
downstream can tell. Keeping one tested implementation of that conversion is
worth more than the convenience of parsing in Python.
"""

from __future__ import annotations

import abc
from dataclasses import dataclass, field


@dataclass(frozen=True)
class PriceBreak:
    qty: int
    unit_price: float
    currency: str


@dataclass
class VendorPart:
    """One part as a vendor describes it, with values left as strings.

    `attrs_raw` is keyed by **spec attribute name** (the adapter's mapping has
    already happened) and valued by the vendor's **untouched string**. Attributes
    the vendor does not publish are simply absent, which the selection engine
    treats as a rejection — the correct outcome for a field a catalogue should
    carry, and the reason `Evidence::Datasheet` exists for fields it should not.
    """

    vendor: str
    vendor_pn: str
    mpn: str
    manufacturer: str
    description: str
    stock: int
    attrs_raw: dict[str, str] = field(default_factory=dict)
    price_breaks: list[PriceBreak] = field(default_factory=list)
    moq: int | None = None
    order_multiple: int | None = None
    lead_time_days: int | None = None
    lifecycle_raw: str | None = None
    status_raw: list[str] = field(default_factory=list)
    datasheet_url: str | None = None

    def unit_price_cents(self) -> int | None:
        """Cheapest single-unit price, in cents, for ranking.

        Qty-1 pricing specifically: this is a hand-assembled small batch, so a
        break at 3000 pieces is not a price we will ever pay, and ranking on it
        would systematically prefer reel-only parts.
        """
        ones = [b for b in self.price_breaks if b.qty <= 1]
        if not ones:
            return None
        return round(min(b.unit_price for b in ones) * 100)


class Adapter(abc.ABC):
    """What a distributor must provide to join the pipeline."""

    #: Stable short name, used as the `distributors` key in the snapshot.
    name: str
    #: Human name, for reports.
    display_name: str
    #: Which warehouse ships, as an ISO country code — the EU-sourcing question.
    warehouse: str

    @abc.abstractmethod
    def available(self) -> bool:
        """Whether credentials for this vendor are present.

        Returning `False` must be cheap and must not raise: the pipeline reports
        an unavailable vendor rather than failing, so a partial run with one
        working adapter is a normal outcome rather than an error.
        """

    @abc.abstractmethod
    def lookup_mpns(self, mpns: list[str]) -> list[VendorPart]:
        """Resolve manufacturer part numbers to fully-populated parts.

        The *verification* path: "is this specific part buyable, at what price".
        """

    @abc.abstractmethod
    def search_category(self, category: str, limit: int) -> list[VendorPart]:
        """Every part this vendor has in a category.

        The *discovery* path, and the one the selection engine exists for:
        filtering 400 candidates is where a constraint earns its keep, and
        verification of 11 already-chosen parts never exercises it.
        """

    # --- Shared helpers -------------------------------------------------

    #: Vendor parameter name -> spec attribute name. Subclasses override.
    #: Keys are compared case-insensitively and with surrounding whitespace
    #: stripped, because catalogues are not consistent about either.
    ATTR_MAP: dict[str, str] = {}

    def map_attrs(self, vendor_params: list[tuple[str, str]]) -> dict[str, str]:
        """Apply `ATTR_MAP`, dropping parameters the spec has no name for.

        Dropping is right: a catalogue carries dozens of parameters per part and
        the spec constrains a handful. Carrying the rest would bloat the export
        without making anything checkable.
        """
        out: dict[str, str] = {}
        lowered = {k.strip().lower(): v for k, v in self.ATTR_MAP.items()}
        for name, value in vendor_params:
            key = lowered.get(name.strip().lower())
            if key and value and value.strip():
                # First non-empty wins: vendors sometimes repeat a parameter with
                # an empty value, and an empty string would read as "published
                # but unknown" when it means "not published".
                out.setdefault(key, value.strip())
        return out
