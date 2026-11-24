"""Per-vendor request pacing, shared by every adapter.

# Why this is not `time.sleep(2)` in a loop

Three vendors, three different caps, and the cheapest one is the tightest:
Mouser allows **30 requests/minute and 1000/day**, where Digi-Key's production
tier is an order of magnitude more generous per minute. A sweep that is polite
enough for Digi-Key will be throttled by Mouser halfway through, and the failure
mode is the one this pipeline keeps finding: a 429 counted as absence reads
exactly like "this vendor does not carry the part".

So pacing is declared per vendor, in one place, next to the evidence for the
number.

# Three distinct mechanisms, because they fail differently

1. **Spacing** (`per_minute`). Enforced as a *minimum interval between calls*
   rather than burst-then-sleep. Bursting 30 calls and sleeping 60 s has the same
   average rate but trips any limiter measuring a sliding window, which is what
   limiters actually do.

2. **A daily budget** (`per_day`). This one cannot live in process memory. The
   cap is per calendar day and a refresh gets run several times a day — an
   in-process counter starts at zero each time and the fourth run of the morning
   discovers the cap by being refused. `spent_today` is passed in from
   `fetch_runs.api_calls`, which the DB has recorded all along.

3. **Backoff** on an actual 429. Honours `Retry-After` when the vendor sends one,
   because a guessed backoff is either rude or slow, and the vendor is telling us
   the answer.

# What this deliberately does not do

It does not decide whether a call is *worth* making — that is staleness, and it
belongs to the fetch planner, which knows what is already in the catalogue. A
limiter that silently skipped requests would make a partial sweep look complete,
which is the one thing every part of this pipeline is built to refuse.
"""

from __future__ import annotations

import time
from dataclasses import dataclass


class BudgetExhausted(RuntimeError):
    """The vendor's daily call budget is spent.

    Raised rather than slept on: the cap resets at midnight, so waiting is not a
    strategy, and continuing would produce 429s that the adapters would have to
    distinguish from real absence all over again.
    """


@dataclass(frozen=True)
class RateLimit:
    """A vendor's published caps, with headroom applied at use.

    `per_minute`/`per_day` are the vendor's own numbers, kept verbatim so they
    can be checked against documentation. `None` means the vendor publishes no
    cap for that window — which is not the same as "no cap", so a default
    interval still applies.
    """

    per_minute: int | None = None
    per_day: int | None = None
    #: Fraction of the daily budget left unspent, so an interactive query is
    #: still possible after a full refresh has run.
    reserve: float = 0.1

    def min_interval(self) -> float:
        if not self.per_minute:
            # No published per-minute cap. 4/s is well inside what any
            # distributor API tolerates and still fast enough for a sweep.
            return 0.25
        # 5% headroom: the vendor's window and ours are not aligned, so pacing
        # at exactly the cap puts every call on the boundary.
        return 60.0 / self.per_minute * 1.05

    def budget(self) -> int | None:
        if not self.per_day:
            return None
        return int(self.per_day * (1.0 - self.reserve))


#: Published caps per vendor.
#:
#: mouser  — 30/minute and 1000/day, stated on the Mouser API subscription page
#:           and in the key's confirmation email. The tightest of the three, and
#:           the reason this module exists.
#: digikey — the production tier's documented ceiling. Measured behaviour matches:
#:           sweeps at ~8 calls/second returned 200s, while the transient 500s
#:           seen earlier were unrelated to rate.
#: tme     — TME publishes no numeric per-minute cap for the v2 API, so the
#:           default interval applies. Left explicit rather than omitted, so the
#:           absence of a number is visible as a fact about TME's docs rather
#:           than an oversight here.
LIMITS: dict[str, RateLimit] = {
    "mouser": RateLimit(per_minute=30, per_day=1000),
    "digikey": RateLimit(per_minute=120, per_day=1000),
    "tme": RateLimit(per_minute=None, per_day=None),
}


def limit_for(vendor: str) -> RateLimit:
    return LIMITS.get(vendor, RateLimit())


class Throttle:
    """Paces one vendor's calls and accounts for them.

    Construct with `spent_today` from the catalogue DB so the daily budget
    survives across process invocations.
    """

    def __init__(self, vendor: str, spent_today: int = 0, sleep=time.sleep,
                 clock=time.monotonic):
        self.vendor = vendor
        self.limit = limit_for(vendor)
        self.spent_today = spent_today
        self.calls = 0
        self._last: float | None = None
        self._sleep = sleep
        self._clock = clock
        #: Set from a vendor's own rate headers when it sends them, so the
        #: declared cap can be checked against what the vendor believes.
        self.reported_remaining: int | None = None

    # --- Budget ---------------------------------------------------------

    def remaining_today(self) -> int | None:
        budget = self.limit.budget()
        if budget is None:
            return None
        return max(0, budget - self.spent_today - self.calls)

    def affordable(self, n: int) -> bool:
        left = self.remaining_today()
        return left is None or left >= n

    # --- Pacing ---------------------------------------------------------

    def acquire(self) -> None:
        """Block until the next call may be made, then count it.

        Counting here rather than after the response is deliberate: a call that
        fails still consumed quota, and a limiter that only counted successes
        would over-spend exactly when things are going wrong.
        """
        left = self.remaining_today()
        if left is not None and left <= 0:
            raise BudgetExhausted(
                f"{self.vendor}: daily budget spent "
                f"({self.spent_today + self.calls} calls today, cap "
                f"{self.limit.per_day} with {int(self.limit.reserve * 100)}% reserved). "
                "Resets at midnight vendor time."
            )
        interval = self.limit.min_interval()
        now = self._clock()
        if self._last is not None:
            wait = interval - (now - self._last)
            if wait > 0:
                self._sleep(wait)
                now = self._clock()
        self._last = now
        self.calls += 1

    def note_headers(self, headers) -> None:
        """Record a vendor's rate headers, if it sends any.

        Digi-Key sends `X-RateLimit-Remaining`; Mouser sends nothing. Treated as
        observation only — never as permission to exceed the declared cap, since
        a header can be absent and its absence must not read as "unlimited".
        """
        if headers is None:
            return
        for key in ("X-RateLimit-Remaining", "x-ratelimit-remaining",
                    "X-RateLimit-Remaining-Day"):
            try:
                raw = headers.get(key)
            except AttributeError:
                return
            if raw is None:
                continue
            try:
                self.reported_remaining = int(str(raw).strip())
            except ValueError:
                pass
            return

    def backoff(self, attempt: int, retry_after=None) -> float:
        """Seconds to wait after a rejected call.

        `retry_after` is the vendor's `Retry-After` header when present: it is
        authoritative, and guessing against it is either rude or needlessly slow.
        """
        if retry_after is not None:
            try:
                secs = float(str(retry_after).strip())
                if secs >= 0:
                    return min(secs, 120.0)
            except ValueError:
                pass
        # Exponential, floored at the pacing interval so backoff is never
        # faster than normal pacing.
        return max(self.limit.min_interval(), 2.0 * (2 ** attempt))

    def plan(self, n_calls: int) -> str:
        """Human description of what `n_calls` will cost, for the run log.

        Printed before a sweep so an operator can see that a job will take four
        minutes *before* it takes four minutes.
        """
        secs = n_calls * self.limit.min_interval()
        left = self.remaining_today()
        budget = "" if left is None else f", {left} of today's budget left"
        return (f"{self.vendor}: {n_calls} call(s) at "
                f"{self.limit.min_interval():.2f}s spacing "
                f"≈ {secs:.0f}s{budget}")
