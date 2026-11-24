"""Canonical vocabulary for enumerated attribute values.

# The problem this solves

Two vendors describe one part in different words. For `mounting`, TME says `SMD`
and Digi-Key says `Surface Mount` — for the *same* part, every time. The spec
constrains `mounting == "SMD"`, so whichever value survived aggregation decided
whether the part qualified, and aggregation was a dict comprehension whose winner
depended on SQL row order. The filter's verdict was nondeterministic.

# Why this is translation and not interpretation

The adapter boundary says Python does transport and NAME mapping, never value
interpretation — unit parsing stays in `dap_hw_spec::parametric` so there is one
tested implementation. Mapping `"Surface Mount"` to `"SMD"` is the same kind of
operation as mapping the field name `"Mounting Type"` to `mounting`: a
declarative vocabulary translation, not a computation. Nothing is parsed, scaled
or inferred.

The alternative was to list every vendor's synonyms in every spec constraint,
which spreads vendor knowledge into the specification — so adding a third
distributor would mean editing requirements rather than adding an adapter. That
is the wrong direction.

# Where it is applied, and what stays raw

At **export**, not in the adapter. The catalogue DB keeps each vendor's exact
words (`attributes.value_raw`) so it remains an audit record of what was actually
said, and the committed raw responses under `hardware/bom/responses/` are the
deeper audit trail. Canonicalisation happens where values are already being
merged across vendors, which is the only place a conflict is even visible.

# Conflicts are reported, not silently resolved

If two vendors still disagree *after* canonicalisation, that is a genuine data
conflict — one of them is wrong about the part — and the export surfaces it
rather than picking a winner quietly.
"""

from __future__ import annotations

# attribute -> {lowercased vendor value: canonical value}
#
# Only enumerated fields belong here. Anything numeric, dimensioned or ranged is
# left untouched for `parametric::derive`, which is the one place units are ever
# interpreted.
SYNONYMS: dict[str, dict[str, str]] = {
    "mounting": {
        "smd": "SMD",
        "surface mount": "SMD",
        "surface mount technology": "SMD",
        "smd/smt": "SMD",
        "tht": "THT",
        "through hole": "THT",
        "through-hole": "THT",
    },
    # The active-vs-passive clock discriminator, which is the one that decides
    # whether a part can drive the DAC and the SAI from one output.
    "part_type": {
        "oscillator": "oscillator",
        "xo": "oscillator",
        "xo (standard)": "oscillator",
        "xo (standard) with standby": "oscillator",
        "crystal": "crystal",
        "ceramic": "ceramic",
        "resonator": "resonator",
    },
    "battery_chemistry": {
        "li-po": "Li-Po",
        "li-pol": "Li-Po",
        "lipo": "Li-Po",
        "lithium polymer": "Li-Po",
        "lithium-polymer": "Li-Po",
        "li-ion": "Li-Ion",
        "ni-mh": "Ni-MH",
        "nimh": "Ni-MH",
    },
    "switch_configuration": {
        "spdt": "SPDT",
        "1 x spdt": "SPDT",
        "spst": "SPST",
        "sp4t": "SP4T",
        "dpdt": "DPDT",
    },
    # Vendors decorate core names differently: TME says 'Cortex M7', Digi-Key
    # says 'ARM® Cortex®-M7'. The spec constrains the concept once; a spelling
    # not listed here passes through verbatim and fails the constraint visibly,
    # which is the intended behaviour for an unrecognised core.
    "cpu_core": {
        "cortex m7": "Cortex-M7",
        "cortex-m7": "Cortex-M7",
        "arm® cortex®-m7": "Cortex-M7",
        "arm cortex-m7": "Cortex-M7",
        "cortex m4": "Cortex-M4",
        "cortex-m4": "Cortex-M4",
        "arm® cortex®-m4": "Cortex-M4",
        "cortex m33": "Cortex-M33",
        "arm® cortex®-m33": "Cortex-M33",
    },
    "data_interface": {
        "i2s": "I2S",
        "i²s": "I2S",
        "i2s/tdm": "I2S",
    },
}


def canonical(attr: str, value: str) -> str:
    """Canonical form of one enumerated value, or the value unchanged.

    Unknown values pass through verbatim. That is deliberate: silently mapping an
    unrecognised token to something familiar would invent agreement that does not
    exist, and a value the spec does not expect should reach the filter and be
    rejected visibly.
    """
    table = SYNONYMS.get(attr.strip().lower())
    if not table:
        return value
    return table.get(value.strip().lower(), value)


def _is_range(v: str) -> bool:
    return any(sep in v for sep in ("~", "...", " - ", " to ", ".."))


def merge_values(attr: str, by_vendor: dict[str, str]) -> tuple[str, list[str]]:
    """Reduce per-vendor values for one attribute to a single value.

    Returns `(value, conflicts)`.

    # Determinism

    The winner is chosen by a stated rule, never by SQL row order, so a re-export
    cannot reshuffle a committed artefact. It is also not chosen by "most stock":
    the value being merged describes the COMPONENT, and whichever vendor holds
    more inventory has no bearing on how a part is mounted.

    # Prefer the range over the nominal

    For non-enumerated fields a range strictly contains more information than a
    scalar, and vendors differ on which they publish: Digi-Key gives the Kyocera
    oscillator's supply as `'1.71V ~ 3.63V'` where TME gives `'3.3V DC'`. Both are
    true; only one lets a containment constraint do real work. (With the nominal,
    `supply_mv_min <= 3300 <= supply_mv_max` passes on a knife-edge because both
    bounds derive from the same number.)

    # Conflicts are only reported for ENUMERATED fields

    String inequality is meaningless for the rest. `'22.5792 MHz'` versus
    `'22.5792MHz'`, or `'-40°C ~ 85°C'` versus `'-40...85°C'`, are the same value
    spelled differently, and `parametric::derive` reconciles them — reporting
    those as conflicts buries the real ones in noise. Numeric disagreement is
    caught downstream, where the constraints compare parsed values rather than
    strings.
    """
    values = {v: raw for v, raw in by_vendor.items() if raw and raw.strip()}
    if not values:
        return "", []

    enumerated = attr.strip().lower() in SYNONYMS
    canon = {v: canonical(attr, raw) for v, raw in values.items()}

    if enumerated:
        distinct = sorted(set(canon.values()))
        winner = canon[sorted(canon)[0]]
        if len(distinct) > 1:
            return winner, [f"{v}={canon[v]!r}" for v in sorted(canon)]
        return winner, []

    # Non-enumerated: prefer a range, then the first vendor alphabetically.
    ranged = sorted(v for v in canon if _is_range(canon[v]))
    winner_vendor = ranged[0] if ranged else sorted(canon)[0]
    return canon[winner_vendor], []
