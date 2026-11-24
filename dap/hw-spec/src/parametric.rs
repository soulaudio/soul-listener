//! Turning distributor parametric strings into the typed attributes the
//! constraints read.
//!
//! # Why this is the part with the logic in it
//!
//! A distributor API does not return `supply_mv_max: 3300`. It returns
//! `"Voltage - Supply: 1.8 V ~ 3.3 V"`, or `"3.3V"`, or `"3V3"`, and the
//! [`select`](crate::select) constraints need a number in a known unit before
//! they can reject anything. Mapping vendor field *names* is trivial renaming;
//! parsing vendor field *values* is where a sourcing pipeline actually goes
//! wrong, and it goes wrong silently in two directions:
//!
//! * **Too strict, visibly.** A unit misread by 1000x rejects every part in the
//!   category. Annoying, obvious, harmless.
//! * **Too lax, invisibly.** The same misread in the other direction accepts a
//!   5 V oscillator onto a 3.3 V board, and nothing downstream notices, because
//!   a number that passed a constraint looks exactly like a number that should
//!   have.
//!
//! So the rule this module is built on: **an ambiguous value is refused, never
//! guessed.** A bare `"3.3"` for a millivolt attribute returns `None`. It is not
//! 3.3 mV and it is not obviously 3300 mV either — it is a value whose unit the
//! vendor did not state, and the only safe reading of an unstated unit is "I do
//! not know", which the selection engine already treats as a rejection
//! (`a_missing_attribute_is_a_rejection_not_a_pass`).
//!
//! That is also why this lives in `dap-hw-spec` rather than in the fetcher. The
//! attribute vocabulary and the constraints that consume it are defined here, so
//! the conversion into that vocabulary belongs here too — and it stays `no_std`
//! and dependency-free, which means it is covered by the same test run as the
//! constraints themselves rather than by a tool nobody runs.
//!
//! # What this deliberately does not do
//!
//! It does not know any vendor's schema. Field plumbing (which JSON key holds
//! the manufacturer part number) is per-vendor and trivial; it belongs in the
//! fetcher, where a recorded response can prove it. Everything here is the shared
//! half that no vendor's documentation is needed to get right.

/// The physical dimension a parsed quantity is expressed in.
///
/// Only the dimensions the spec's constraints actually use. Adding one means
/// adding a constraint that needs it, not anticipating a future need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    /// Hz, after SI-prefix expansion.
    Frequency,
    /// Millivolts.
    Voltage,
    /// Millimetres.
    Length,
    /// Milliamp-hours.
    Charge,
    /// Watt-hours.
    Energy,
    /// dBc/Hz — already logarithmic, so no prefix scaling applies.
    PhaseNoise,
    /// A plain count with no unit (channels, cells in series).
    Count,
    /// Bytes, binary-prefixed. Memory sizes are conventionally powers of two and
    /// catalogues write them that way, so `MB` here is 1024^2 rather than 10^6.
    /// The distinction does not change any `AtLeast(1 MB)` style answer, but
    /// recording which convention is in use costs nothing and prevents a later
    /// reader assuming the other one.
    Bytes,
}

/// An SI prefix and its multiplier.
///
/// `K` is accepted alongside `k` because catalogues are inconsistent about it,
/// and `M`/`m` are case-sensitive on purpose: confusing mega with milli is a
/// 10^9 error, so the one place case must be respected is exactly here.
const PREFIXES: &[(char, f64)] = &[
    ('p', 1e-12),
    ('n', 1e-9),
    ('u', 1e-6),
    ('m', 1e-3),
    ('k', 1e3),
    ('K', 1e3),
    ('M', 1e6),
    ('G', 1e9),
];

/// Unit suffixes, longest-first so `"Hz"` is not read as `"H"`.
///
/// Each entry maps a written unit to the dimension it measures and the factor
/// that converts it to that dimension's canonical unit.
const UNITS: &[(&str, Dimension, f64)] = &[
    ("dBc/Hz", Dimension::PhaseNoise, 1.0),
    ("GB", Dimension::Bytes, 1024.0 * 1024.0 * 1024.0),
    ("MB", Dimension::Bytes, 1024.0 * 1024.0),
    ("kB", Dimension::Bytes, 1024.0),
    ("KB", Dimension::Bytes, 1024.0),
    ("dbc/hz", Dimension::PhaseNoise, 1.0),
    ("Wh", Dimension::Energy, 1.0),
    ("wh", Dimension::Energy, 1.0),
    ("Ah", Dimension::Charge, 1000.0),
    ("ah", Dimension::Charge, 1000.0),
    ("Hz", Dimension::Frequency, 1.0),
    ("hz", Dimension::Frequency, 1.0),
    ("mm", Dimension::Length, 1.0),
    ("cm", Dimension::Length, 10.0),
    ("in", Dimension::Length, 25.4),
    ("V", Dimension::Voltage, 1000.0),
    ("v", Dimension::Voltage, 1000.0),
];

/// A quantity parsed out of a catalogue string.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    /// The value in the dimension's canonical unit (Hz, mV, mm, mAh, Wh, dBc/Hz).
    pub value: f64,
    pub dimension: Dimension,
}

/// Parse one quantity, requiring an explicit unit.
///
/// Handles the shapes catalogues actually produce: `"3.3V"`, `"3.3 V"`,
/// `"22.5792MHz"`, `"-142 dBc/Hz"`, `"1100 mAh"`, `"4.07Wh"`, and the
/// engineering shorthand `"3V3"`.
///
/// Returns `None` when there is no unit, when the number does not parse, or when
/// the string carries a *range* — see [`parse_range_max`] for that, deliberately
/// a separate function so a range can never be silently read as a single value.
///
/// # The millivolt trap
///
/// `parse_quantity("3.3")` is `None`, not `Some(3.3 mV)` and not `Some(3300 mV)`.
/// The number alone cannot distinguish a 3.3 V part from a 3.3 mV one, and
/// guessing is how a 5 V oscillator reaches a 3.3 V board.
#[must_use]
pub fn parse_quantity(s: &str) -> Option<Quantity> {
    let t = strip_trailing_qualifiers(s);
    if t.is_empty() {
        return None;
    }
    // A range is a different thing and must be asked for explicitly.
    if is_range(t) {
        return None;
    }

    // Engineering shorthand: 3V3 == 3.3 V, 1k2 == 1200.
    if let Some(q) = parse_infix_shorthand(t) {
        return Some(q);
    }

    // Longest unit first, so "Hz" wins over any single-letter match.
    for (unit, dim, to_canonical) in UNITS {
        let Some(head) = t.strip_suffix(unit) else {
            continue;
        };
        let head = head.trim();
        if head.is_empty() {
            continue;
        }

        // An SI prefix sits between the number and the unit. dBc/Hz is
        // logarithmic, so a prefix there would be meaningless and is refused.
        let (num_str, prefix_factor) = match head.chars().last() {
            Some(c) if PREFIXES.iter().any(|(p, _)| *p == c) => {
                if *dim == Dimension::PhaseNoise {
                    return None;
                }
                let f = PREFIXES
                    .iter()
                    .find(|(p, _)| *p == c)
                    .map(|(_, f)| *f)
                    .unwrap_or(1.0);
                (&head[..head.len() - c.len_utf8()], f)
            }
            _ => (head, 1.0),
        };

        let n: f64 = num_str.trim().parse().ok()?;
        return Some(Quantity {
            value: n * prefix_factor * to_canonical,
            dimension: *dim,
        });
    }
    None
}

/// Parse a bare count, which is the one case where no unit is correct.
///
/// `"2"` channels, `"1"` cell in series. Refuses anything with a unit attached,
/// because a unit means the value is not a count and the caller has the wrong
/// attribute.
#[must_use]
pub fn parse_count(s: &str) -> Option<Quantity> {
    let t = s.trim();
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse().ok().map(|value| Quantity {
        value,
        dimension: Dimension::Count,
    })
}

/// Does this string express a range rather than a single value?
fn is_range(t: &str) -> bool {
    t.contains('~') || t.contains(" - ") || t.contains(" to ") || t.contains("..")
}

/// Drop trailing descriptor words that are not units.
///
/// Required by real data rather than anticipated: TME publishes supply voltage as
/// `"1.65...5.5V DC"` and memory as `"2MB FLASH"`. The `DC` and `FLASH` are
/// descriptors, and without removing them nothing matches a unit suffix and the
/// whole value is refused — which is how the first run against live data produced
/// `None` for every parametric string.
///
/// A token is only dropped when it is purely alphabetic, is not itself a known
/// unit, and is not the only token left. So `"3.3 V"` keeps its `V`, and `"SMD"`
/// is returned untouched rather than emptied.
fn strip_trailing_qualifiers(t: &str) -> &str {
    let mut cur = t.trim();
    loop {
        let Some((head, last)) = cur.rsplit_once(char::is_whitespace) else {
            return cur;
        };
        let head = head.trim_end();
        if head.is_empty()
            || !last.chars().all(|c| c.is_ascii_alphabetic())
            // `ends_with`, not equality: a token is usually PREFIX+UNIT, so
            // "MHz" and "mAh" must be recognised as units even though neither
            // string is in UNITS. Comparing for equality here stripped them as
            // descriptors and made every prefixed quantity unparseable — caught
            // by the existing frequency and charge tests.
            || UNITS.iter().any(|(u, _, _)| last.ends_with(u))
        {
            return cur;
        }
        cur = head;
    }
}

/// The upper bound of a range like `"1.8 V ~ 3.3 V"`.
///
/// Separate from [`parse_quantity`] on purpose. Supply voltages are routinely
/// published as an operating range, and the spec's constraint is
/// `supply_mv_max <= 3300` — so the *maximum* is the figure that matters, and
/// reading the minimum instead would pass a 5 V part whose range starts at 1.8 V.
/// Requiring the caller to ask for a range makes that choice explicit at the call
/// site rather than hidden in a parser.
#[must_use]
pub fn parse_range_max(s: &str) -> Option<Quantity> {
    range_bound(s, Bound::Max)
}

/// The lower bound of a range. See [`parse_range_max`] — the two exist
/// separately so a caller cannot get the bound it did not mean.
#[must_use]
pub fn parse_range_min(s: &str) -> Option<Quantity> {
    range_bound(s, Bound::Min)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bound {
    Min,
    Max,
}

fn range_bound(s: &str, want: Bound) -> Option<Quantity> {
    let t = strip_trailing_qualifiers(s);
    if !is_range(t) {
        return None;
    }
    // Longest separator first: "..." must be tried before "..", or TME's
    // `1.65...5.5V` splits into `1.65` and `.5.5V` and the second side is
    // unparseable.
    let sep = if t.contains('~') {
        "~"
    } else if t.contains(" to ") {
        " to "
    } else if t.contains("...") {
        "..."
    } else if t.contains("..") {
        ".."
    } else {
        " - "
    };

    // `"1.8 ~ 3.3 V"` is a common shape, so a side with no unit of its own
    // inherits the one the other side states. The unit is resolved first, from
    // whichever side carries it, and then applied to any bare side — which keeps
    // this allocation-free (this crate has no dependencies, deliberately, so
    // there is no string type to build `"1.8" + "V"` into).
    let stated = t.split(sep).find_map(|p| units_of(p.trim()));

    let mut best: Option<Quantity> = None;
    for piece in t.split(sep) {
        let p = piece.trim();
        let q = match parse_quantity(p) {
            Some(q) => q,
            None => {
                // A bare side: a plain number, interpreted in the stated unit.
                let (dim, to_canonical) = stated?;
                let n: f64 = p.parse().ok()?;
                Quantity {
                    value: n * to_canonical,
                    dimension: dim,
                }
            }
        };
        best = match (best, want) {
            (Some(b), Bound::Max) if b.value >= q.value => Some(b),
            (Some(b), Bound::Min) if b.value <= q.value => Some(b),
            _ => Some(q),
        };
    }
    best
}

/// The dimension and canonical-conversion factor of the unit a string ends with.
fn units_of(t: &str) -> Option<(Dimension, f64)> {
    UNITS
        .iter()
        .find(|(unit, _, _)| t.ends_with(unit))
        .map(|(_, dim, to_canonical)| (*dim, *to_canonical))
}

/// Engineering infix shorthand: `3V3` → 3.3 V, `1k2` → 1200.
fn parse_infix_shorthand(t: &str) -> Option<Quantity> {
    let bytes = t.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if i == 0 || i + 1 >= bytes.len() {
            continue;
        }
        let c = b as char;
        let unit = UNITS
            .iter()
            .find(|(u, _, _)| u.len() == 1 && u.as_bytes()[0] == b);
        let prefix = PREFIXES.iter().find(|(p, _)| *p == c);
        // Only a single-character unit or an SI prefix can sit infix.
        let (dim, to_canonical, factor) = match (unit, prefix) {
            (Some((_, d, f)), _) => (*d, *f, 1.0),
            (None, Some((_, f))) => (Dimension::Count, 1.0, *f),
            _ => continue,
        };
        let (head, tail) = (&t[..i], &t[i + 1..]);
        if !head.chars().all(|c| c.is_ascii_digit() || c == '-')
            || !tail.chars().all(|c| c.is_ascii_digit())
        {
            continue;
        }
        // Reassemble as "head.tail" and scale.
        //
        // `f64::powi` is a `std` method, so it is unavailable here — this crate
        // is `no_std` with no dependencies (not even libm). A multiply loop is
        // exact for the small exponents a part number can carry, and it is also
        // how this caught itself: `cargo test` compiled `powi` fine because the
        // test harness links std, and only the real lib build failed.
        let whole: f64 = head.parse().ok()?;
        let frac: f64 = tail.parse().ok()?;
        let mut scale = 1.0f64;
        for _ in 0..tail.len() {
            scale *= 10.0;
        }
        let combined = whole + frac / scale;
        return Some(Quantity {
            value: combined * to_canonical * factor,
            dimension: dim,
        });
    }
    None
}

/// Parse a catalogue boolean.
///
/// Distributors publish these as free text and disagree on spelling, so the
/// accepted vocabulary is explicit. Anything unrecognised is `None` rather than
/// `false`: "unparsed" and "the vendor said no" must not collapse into the same
/// value, because several constraints are
/// [`IsTrue`](crate::select::Predicate::IsTrue) and a wrong `false` silently
/// rejects a good part while a wrong `true` silently accepts a bad one.
#[must_use]
pub fn parse_bool(s: &str) -> Option<bool> {
    let t = s.trim();
    const TRUE: &[&str] = &[
        "true",
        "yes",
        "y",
        "1",
        "compliant",
        "rohs compliant",
        "active",
        "available",
        "included",
    ];
    const FALSE: &[&str] = &[
        "false",
        "no",
        "n",
        "0",
        "non-compliant",
        "not compliant",
        "unavailable",
        "none",
        "not included",
    ];
    if TRUE.iter().any(|v| v.eq_ignore_ascii_case(t)) {
        return Some(true);
    }
    if FALSE.iter().any(|v| v.eq_ignore_ascii_case(t)) {
        return Some(false);
    }
    None
}

/// Convert a parsed quantity into the value a named attribute expects.
///
/// The attribute names are the ones the spec's constraints use. The dimension
/// check is the point: asking for `supply_mv_max` and handing it a frequency
/// returns `None` instead of a number, so a vendor field mapped to the wrong
/// attribute fails loudly at the boundary rather than producing a plausible
/// figure that no constraint can tell is wrong.
#[must_use]
pub fn attribute_value(attr: &str, q: Quantity) -> Option<f64> {
    let expected = expected_dimension(attr)?;
    if q.dimension != expected {
        return None;
    }
    Some(q.value)
}

/// Which dimension an attribute in the spec's vocabulary is measured in.
#[must_use]
pub fn expected_dimension(attr: &str) -> Option<Dimension> {
    let a = attr;
    Some(match () {
        _ if eq(a, "frequency_hz") || eq(a, "max_freq_hz") => Dimension::Frequency,
        _ if eq(a, "supply_mv_max") || eq(a, "supply_mv_min") => Dimension::Voltage,
        _ if eq(a, "rated_voltage_mv") => Dimension::Voltage,
        _ if eq(a, "sram_bytes") || eq(a, "flash_bytes") => Dimension::Bytes,
        _ if eq(a, "core_clock_hz") => Dimension::Frequency,
        _ if eq(a, "phase_noise_dbc_1khz") => Dimension::PhaseNoise,
        _ if eq(a, "capacity_mah") => Dimension::Charge,
        _ if eq(a, "watt_hours") => Dimension::Energy,
        _ if eq(a, "thickness_mm") || eq(a, "width_mm") || eq(a, "length_mm") => Dimension::Length,
        _ if eq(a, "channels") || eq(a, "cells_series") => Dimension::Count,
        _ if eq(a, "resolution_bits") => Dimension::Count,
        _ if eq(a, "dynamic_range_db") => Dimension::Count,
        // Minimum order quantity. A count, and a constraint rather than a
        // ranking criterion — see the oscillator requirement for why a 1000-piece
        // reel is not merely "worse" for a hand-assembled batch.
        _ if eq(a, "moq") || eq(a, "order_multiple") => Dimension::Count,
        // Mechanical durability of a connector, as a plain count of mating
        // cycles. Datasheet-only on the microSD requirement — no distributor
        // publishes it — but the dimension is still required, because
        // `every_numeric_constraint_attribute_has_a_known_dimension` refuses a
        // numeric constraint whose string a fetcher could never convert. That
        // gate is the reason this entry exists rather than being noticed later
        // by a constraint silently rejecting every part.
        _ if eq(a, "insertion_cycles") => Dimension::Count,
        // Height above board, for a card connector. Mapped from both vendors
        // but NOT yet constrained: Digi-Key states it dual-unit as
        // '0.104" (2.65mm)', which `parse_quantity` does not read, so a
        // constraint resting on it today would reject every Digi-Key socket for
        // an unparseable value. This is the hook the deferred size filtering
        // needs — see MICROSD_CONSTRAINTS and the battery's thickness_mm.
        _ if eq(a, "height_mm") => Dimension::Length,
        _ => return None,
    })
}

fn eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Case-insensitive suffix test without allocating.
///
/// `str::to_ascii_lowercase` returns a `String`, and this crate is `no_std` with
/// no `alloc`. Compared over bytes rather than by slicing so a non-ASCII
/// attribute name cannot panic on a char boundary.
fn ends_with_ci(s: &str, suffix: &str) -> bool {
    let (sb, fb) = (s.as_bytes(), suffix.as_bytes());
    sb.len() > fb.len()
        && sb[sb.len() - fb.len()..]
            .iter()
            .zip(fb)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// Derive the value of a named spec attribute from one raw catalogue string.
///
/// This is the single entry point a fetcher's output should go through, and it
/// exists so that **no vendor-side code ever interprets a value**. An adapter
/// maps a vendor's parameter NAME onto a spec attribute name and hands over the
/// string untouched; everything about what the string means happens here, in the
/// crate the constraints live in and under the same tests.
///
/// # The `_min`/`_max` convention
///
/// Distributors publish one field — `"Supply voltage: 1.65...5.5V DC"` — where
/// the spec needs two bounds, because the real requirement is that the operating
/// range *contains* the board's rail. So an attribute whose name ends `_min` or
/// `_max` is derived from whichever bound of the published range it asks for, and
/// a single-valued string answers both:
///
/// ```
/// use dap_hw_spec::parametric::derive;
/// assert_eq!(derive("supply_mv_min", "1.65...5.5V DC"), Some(1650.0));
/// assert_eq!(derive("supply_mv_max", "1.65...5.5V DC"), Some(5500.0));
/// // A single rated value answers either bound.
/// assert_eq!(derive("supply_mv_max", "3.3V"), Some(3300.0));
/// ```
///
/// Returns `None` for anything ambiguous, unparseable, or of the wrong
/// dimension — the engine then treats the attribute as missing, which is a
/// rejection. That is deliberate: see the module docs on why an unstated unit
/// must never be guessed.
#[must_use]
pub fn derive(attr: &str, raw: &str) -> Option<f64> {
    let dim = expected_dimension(attr)?;

    if dim == Dimension::Count {
        return parse_count(raw).and_then(|q| attribute_value(attr, q));
    }

    // A bound asked of a range.
    let bound = if ends_with_ci(attr, "_min") {
        Some(Bound::Min)
    } else if ends_with_ci(attr, "_max") {
        Some(Bound::Max)
    } else {
        None
    };

    let q = match bound {
        // Try the range first; fall back to a single value, which answers either
        // bound for a part quoting one rating rather than a span.
        Some(b) => range_bound(raw, b).or_else(|| parse_quantity(raw)),
        None => parse_quantity(raw),
    }?;

    if q.dimension != dim {
        return None;
    }
    Some(q.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(s: &str) -> Option<f64> {
        parse_quantity(s).map(|q| q.value)
    }

    /// The headline case: SI prefixes on frequencies, which is how every
    /// oscillator in the catalogue will arrive.
    #[test]
    fn frequencies_expand_si_prefixes_to_hz() {
        assert_eq!(val("22.5792MHz"), Some(22_579_200.0));
        assert_eq!(val("22.5792 MHz"), Some(22_579_200.0));
        assert_eq!(val("24.576 MHz"), Some(24_576_000.0));
        assert_eq!(val("22579200 Hz"), Some(22_579_200.0));
        assert_eq!(val("49.152 MHz"), Some(49_152_000.0));
        // Sloppy unit casing is tolerated; sloppy PREFIX casing is not. "Mhz" is
        // a common misspelling of megahertz and never means millihertz in a
        // parts catalogue, so reading it as mega is both safe and useful. The
        // case that must stay significant is the prefix itself — see
        // `mega_and_milli_are_not_confused`.
        assert_eq!(val("200 Mhz"), Some(200_000_000.0));
        assert_eq!(val("200 MHz"), Some(200_000_000.0));
        assert_eq!(
            val("200 mHz"),
            Some(0.2),
            "milli, because the prefix is lower-case"
        );
    }

    /// Mega and milli differ by 10^9, so the one place case must matter is here.
    #[test]
    fn mega_and_milli_are_not_confused() {
        let mega = parse_quantity("1 MHz").expect("1 MHz parses");
        let milli = parse_quantity("1 mHz").expect("1 mHz parses");
        assert_eq!(mega.value, 1e6);
        assert_eq!(milli.value, 1e-3);
        assert!(
            (mega.value / milli.value - 1e9).abs() < 1.0,
            "a case-insensitive prefix table would make these equal, which is a 10^9 error in a \
             clock frequency"
        );
    }

    /// **The trap this module exists for.** A unitless number is refused.
    #[test]
    fn a_number_without_a_unit_is_refused_rather_than_guessed() {
        assert_eq!(val("3.3"), None);
        assert_eq!(val("3300"), None);
        assert_eq!(val(""), None);
        assert_eq!(val("   "), None);

        // And the consequence, stated as the assertion: no unitless string can
        // become a voltage, so it cannot pass supply_mv_max <= 3300 by accident.
        assert_eq!(
            parse_quantity("5").and_then(|q| attribute_value("supply_mv_max", q)),
            None,
            "a bare '5' must not become 5 mV and sail through a 3300 mV ceiling — that is how a \
             5 V part reaches a 3.3 V board"
        );
    }

    /// Voltages canonicalise to millivolts, because that is what the constraint
    /// is written in.
    #[test]
    fn voltages_become_millivolts() {
        assert_eq!(val("3.3V"), Some(3300.0));
        assert_eq!(val("3.3 V"), Some(3300.0));
        assert_eq!(val("5V"), Some(5000.0));
        assert_eq!(val("1.8 v"), Some(1800.0));
        // 3V3 shorthand.
        assert_eq!(val("3V3"), Some(3300.0));
        assert_eq!(val("1V8"), Some(1800.0));
    }

    /// A supply RANGE must yield its maximum, or the ceiling check is inverted.
    #[test]
    fn a_supply_range_yields_its_maximum_not_its_minimum() {
        // The shape distributors publish.
        let q = parse_range_max("1.8 V ~ 3.3 V").expect("a two-sided range parses");
        assert_eq!(q.value, 3300.0);
        assert_eq!(q.dimension, Dimension::Voltage);

        // A bare left side borrows the unit from the right.
        assert_eq!(
            parse_range_max("1.8 ~ 3.3 V").map(|q| q.value),
            Some(3300.0)
        );

        // The inversion that matters: a 5 V part whose range starts low must
        // report 5000, not 1800.
        assert_eq!(
            parse_range_max("1.8 V ~ 5 V").map(|q| q.value),
            Some(5000.0)
        );

        // And a range is never silently read as a single value.
        assert_eq!(
            val("1.8 V ~ 5 V"),
            None,
            "parse_quantity must refuse a range so the caller has to choose which bound it means"
        );
    }

    /// Phase noise is logarithmic: negative, and no prefix scaling.
    #[test]
    fn phase_noise_keeps_its_sign_and_refuses_prefixes() {
        assert_eq!(val("-142 dBc/Hz"), Some(-142.0));
        assert_eq!(val("-138dBc/Hz"), Some(-138.0));
        assert_eq!(
            val("-142 mdBc/Hz"),
            None,
            "an SI prefix on a logarithmic unit is meaningless and must be refused rather than \
             scaled"
        );
    }

    /// Battery figures.
    #[test]
    fn charge_and_energy_canonicalise() {
        assert_eq!(val("1100 mAh"), Some(1100.0));
        assert_eq!(val("1.1 Ah"), Some(1100.0));
        assert_eq!(val("4.07 Wh"), Some(4.07));
        assert_eq!(val("6.2 mm"), Some(6.2));
        assert_eq!(val("0.62 cm"), Some(6.2));
    }

    /// Counts are the one case where a missing unit is correct — and a unit
    /// means the caller has the wrong attribute.
    #[test]
    fn counts_accept_bare_integers_and_reject_units() {
        assert_eq!(parse_count("2").map(|q| q.value), Some(2.0));
        assert_eq!(parse_count("1").map(|q| q.value), Some(1.0));
        assert_eq!(parse_count("2 channels"), None);
        assert_eq!(parse_count("3.3"), None, "a count is an integer");
    }

    /// A value mapped to the wrong attribute fails at the boundary.
    #[test]
    fn a_dimension_mismatch_is_refused_at_the_attribute_boundary() {
        let freq = parse_quantity("22.5792 MHz").expect("parses");
        assert_eq!(attribute_value("frequency_hz", freq), Some(22_579_200.0));
        assert_eq!(
            attribute_value("supply_mv_max", freq),
            None,
            "a frequency mapped onto the supply attribute must be refused — 22579200 would \
             otherwise look like a perfectly ordinary millivolt figure to a ceiling check, and no \
             constraint could tell it was wrong"
        );
        assert_eq!(
            attribute_value("phase_noise_dbc_1khz", freq),
            None,
            "and a positive 22.6 million would pass an AtMost(-130) check... no: it would fail, \
             but for the wrong reason, which is just as bad for diagnosis"
        );
    }

    /// Unknown attributes have no dimension, so nothing can be coerced into them.
    #[test]
    fn an_unknown_attribute_has_no_dimension() {
        assert_eq!(expected_dimension("colour"), None);
        let q = parse_quantity("3.3 V").expect("parses");
        assert_eq!(attribute_value("colour", q), None);
    }

    /// Every attribute the spec constrains numerically must be parseable.
    ///
    /// This is the gate that keeps this module in step with the constraints: add
    /// a numeric constraint on a new attribute and this fails until the attribute
    /// has a dimension, rather than the fetcher silently producing nothing for it.
    #[test]
    fn every_numeric_constraint_attribute_has_a_known_dimension() {
        use crate::select::Predicate;
        for spec in [&crate::v2::V2, &crate::v1::V1] {
            for req in spec.parts {
                for c in req.constraints {
                    let numeric = matches!(
                        c.pred,
                        Predicate::NumEq { .. } | Predicate::AtMost(_) | Predicate::AtLeast(_)
                    );
                    if !numeric {
                        continue;
                    }
                    assert!(
                        expected_dimension(c.attr).is_some(),
                        "requirement '{}' constrains '{}' numerically, but parametric.rs has no \
                         dimension for it — so a fetcher could not turn a vendor's string into \
                         that attribute, and the constraint would reject every part for a missing \
                         attribute. Add it to expected_dimension (and a unit to UNITS if the \
                         dimension is new).",
                        req.id,
                        c.attr
                    );
                }
            }
        }
    }

    /// **Every parametric string TME actually returned**, pinned.
    ///
    /// Not invented examples: these are lifted verbatim from
    /// `hardware/bom/responses/tme/parameters-0.json`, which is a recorded live
    /// response. The first run of the parser against them returned `None` for
    /// every single one — the separator was `...` rather than `~`, and values
    /// carried trailing descriptors (`DC`, `FLASH`, `SRAM`) that no unit suffix
    /// could match.
    ///
    /// That failure is the reason this test exists in this form. A parser tested
    /// only against strings its author imagined will pass and then read nothing.
    #[test]
    fn the_strings_a_real_distributor_returned_all_parse() {
        // Supply voltage, both parts. `...` separator plus a ` DC` qualifier.
        assert_eq!(
            parse_range_max("1.65...5.5V DC").map(|q| q.value),
            Some(5500.0),
            "the TI mux is rated 1.65-5.5 V; the MAXIMUM is 5500 mV"
        );
        assert_eq!(
            parse_range_min("1.65...5.5V DC").map(|q| q.value),
            Some(1650.0),
            "and the minimum is 1650 mV — needed because the real constraint is \
             that the operating range CONTAINS 3.3 V"
        );
        assert_eq!(
            parse_range_max("1.62...3.6V DC").map(|q| q.value),
            Some(3600.0)
        );
        assert_eq!(
            parse_range_min("1.62...3.6V DC").map(|q| q.value),
            Some(1620.0)
        );

        // The STM32's clock and memory, which make the MCU requirement
        // machine-checkable instead of prose-only.
        assert_eq!(parse_quantity("280MHz").map(|q| q.value), Some(280e6));
        assert_eq!(
            parse_quantity("1.4MB SRAM").map(|q| q.value),
            Some(1.4 * 1024.0 * 1024.0),
            "the trailing descriptor must be dropped, and MB is binary here"
        );
        assert_eq!(
            parse_quantity("2MB FLASH").map(|q| q.value),
            Some(2.0 * 1024.0 * 1024.0)
        );

        // Counts.
        assert_eq!(parse_count("1").map(|q| q.value), Some(1.0));
        assert_eq!(parse_count("112").map(|q| q.value), Some(112.0));

        // Text values must NOT be mistaken for quantities. `SMD` is a single
        // alphabetic token, so the qualifier stripper has to leave it alone
        // rather than reducing it to nothing.
        for text in [
            "SMD",
            "SPDT",
            "SC70-6",
            "LQFP144",
            "Cortex M7",
            "analog switch",
        ] {
            assert_eq!(
                parse_quantity(text),
                None,
                "{text:?} is a text attribute and must not parse as a number"
            );
        }
    }

    /// Stripping qualifiers must not eat a real unit.
    #[test]
    fn a_trailing_unit_is_never_stripped_as_a_qualifier() {
        assert_eq!(parse_quantity("3.3 V").map(|q| q.value), Some(3300.0));
        assert_eq!(
            parse_quantity("22.5792 MHz").map(|q| q.value),
            Some(22_579_200.0)
        );
        assert_eq!(parse_quantity("1100 mAh").map(|q| q.value), Some(1100.0));
        // And a lone qualifier is returned untouched rather than emptied.
        assert_eq!(strip_trailing_qualifiers("SMD"), "SMD");
        assert_eq!(strip_trailing_qualifiers("1.65...5.5V DC"), "1.65...5.5V");
        assert_eq!(strip_trailing_qualifiers("2MB FLASH"), "2MB");
    }

    /// `...` must be tried before `..`, or the right side is unparseable.
    #[test]
    fn the_three_dot_separator_is_matched_before_the_two_dot_one() {
        // Splitting "1.65...5.5V" on ".." yields "1.65" and ".5.5V" — the second
        // is not a number, so the whole range would be refused.
        assert_eq!(
            parse_range_max("1.65...5.5V").map(|q| q.value),
            Some(5500.0)
        );
        // The two-dot form still works for anything that uses it.
        assert_eq!(parse_range_max("1.8..3.3V").map(|q| q.value), Some(3300.0));
    }

    /// Byte sizes use the binary convention, and say so.
    #[test]
    fn memory_sizes_are_binary() {
        assert_eq!(parse_quantity("1kB").map(|q| q.value), Some(1024.0));
        assert_eq!(parse_quantity("2MB").map(|q| q.value), Some(2_097_152.0));
        assert_eq!(
            parse_quantity("1GB").map(|q| q.value),
            Some(1_073_741_824.0)
        );
        assert_eq!(
            parse_quantity("2MB").map(|q| q.dimension),
            Some(Dimension::Bytes)
        );
        // A byte size must not be coerced into a frequency attribute.
        let q = parse_quantity("2MB").expect("parses");
        assert_eq!(attribute_value("core_clock_hz", q), None);
        assert_eq!(attribute_value("flash_bytes", q), Some(2_097_152.0));
    }

    /// `derive` is the whole vendor-facing surface, pinned against real strings.
    #[test]
    fn derive_turns_real_catalogue_strings_into_spec_attributes() {
        // One published range answers both bounds — the thing that makes
        // containment checkable from a single vendor field.
        assert_eq!(derive("supply_mv_min", "1.65...5.5V DC"), Some(1650.0));
        assert_eq!(derive("supply_mv_max", "1.65...5.5V DC"), Some(5500.0));
        assert_eq!(derive("supply_mv_min", "1.62...3.6V DC"), Some(1620.0));
        assert_eq!(derive("supply_mv_max", "1.62...3.6V DC"), Some(3600.0));

        // A single rated value answers either bound.
        assert_eq!(derive("supply_mv_max", "3.3V"), Some(3300.0));
        assert_eq!(derive("supply_mv_min", "3.3V"), Some(3300.0));

        // Counts, frequencies, byte sizes.
        assert_eq!(derive("channels", "1"), Some(1.0));
        assert_eq!(derive("core_clock_hz", "280MHz"), Some(280e6));
        assert_eq!(
            derive("sram_bytes", "1.4MB SRAM"),
            Some(1.4 * 1024.0 * 1024.0)
        );
        assert_eq!(
            derive("flash_bytes", "2MB FLASH"),
            Some(2.0 * 1024.0 * 1024.0)
        );

        // Wrong dimension, unknown attribute, and text all refuse.
        assert_eq!(derive("supply_mv_max", "280MHz"), None);
        assert_eq!(derive("colour", "3.3V"), None);
        assert_eq!(derive("channels", "SPDT"), None);
        assert_eq!(derive("supply_mv_max", ""), None);

        // And the trap: a bare number cannot become a voltage.
        assert_eq!(
            derive("supply_mv_max", "3.3"),
            None,
            "a unitless value must stay unknown rather than being guessed into the attribute's \
             canonical unit"
        );
    }

    /// Catalogue booleans, and the refusal that matters.
    #[test]
    fn booleans_parse_known_vocabulary_and_refuse_the_rest() {
        assert_eq!(parse_bool("Yes"), Some(true));
        assert_eq!(parse_bool("RoHS Compliant"), Some(true));
        assert_eq!(parse_bool("No"), Some(false));
        assert_eq!(parse_bool("Non-Compliant"), Some(false));
        assert_eq!(
            parse_bool("Compliant by Exemption"),
            None,
            "an unrecognised compliance phrase must not become `false`: that would silently \
             reject a good part, and the mirror case would silently accept a bad one"
        );
        assert_eq!(parse_bool(""), None);
    }
}
