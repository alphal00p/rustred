"""Argument plumbing for native whole-query finite replay; no algebra or admission.

The optional work allowances never change the mathematical input scope. Native
enumeration either covers the entire eligible region or uses the ordinary path.
Only the resumable initial-queries mode is exposed here; legacy single-root
experiments remain available directly from the native CLI.
"""
MODE = "finite-replay-initial-queries"
CAPS = tuple("finite-replay-max-" + suffix for suffix in (
    "nodes", "rule-applications", "transport-calls", "transport-operations",
    "transport-endpoints", "coalescing-additions", "positive-layers",
    "seed-points", "seed-bytes",
))
REDUCTION_CAPS = tuple("reduction-max-" + suffix for suffix in (
    "rule-applications", "pending-frames", "coalescing-additions",
))
OPTIONS = (MODE, *CAPS, *REDUCTION_CAPS)
KEYS = tuple(name.replace("-", "_") for name in OPTIONS)


def nonnegative(text):
    value = int(text)
    if value < 0:
        raise ValueError("finite replay allowances must be nonnegative")
    return value


def add_arguments(parser, domain):
    parser.add_argument("--" + MODE, action=domain.StoreTrueOnce, nargs=0,
                        default=None, help="opt into whole initial-query finite replay; requires an epoch checkpoint; frozen on resume")
    for name in (*CAPS, *REDUCTION_CAPS):
        parser.add_argument("--" + name, type=nonnegative, action=domain.StoreOnce,
                            help="explicit native work allowance for finite-query mode; zero is meaningful; not an input bound")


def validate(options, *, symbolic=True, checkpoint=True):
    enabled = options.get(KEYS[0])
    if enabled is not None and type(enabled) is not bool:
        raise ValueError("finite-query mode must be boolean")
    for key in KEYS[1:]:
        value = options.get(key)
        if value is not None and (type(value) is not int or value < 0):
            raise ValueError("finite-query work allowances must be nonnegative integers")
        if value is not None and not enabled:
            raise ValueError("finite-query work allowances require --" + MODE)
    if not enabled:
        return
    if not symbolic or not checkpoint or options.get("publication_policy") != "epoch":
        raise ValueError("finite initial queries require symbolic queries and an epoch checkpoint")
    if (not options.get("epoch_rolling") or options.get("epoch_dispatch") not in (None, "fifo")
            or options.get("epoch_publication_order") not in (None, "oldest-prefix")):
        raise ValueError("finite initial queries currently require rolling FIFO/oldest-prefix execution")
    if options.get("auto_rescue"):
        raise ValueError("finite initial queries support scope extension, not automatic rescue/quarantine")


def arguments(options):
    if not options.get(KEYS[0]):
        return []
    command = ["--" + MODE]
    for name, key in zip(OPTIONS[1:], KEYS[1:]):
        value = options.get(key)
        if value is not None:
            command.extend(("--" + name, str(value)))
    return command


def validate_frozen(options, command):
    validate(options)
    for index, (name, key) in enumerate(zip(OPTIONS, KEYS)):
        flag = "--" + name
        positions = [i for i, token in enumerate(command) if token == flag]
        value = options.get(key)
        expected = bool(value) if index == 0 else value is not None
        if len(positions) != int(expected) or any(
                isinstance(token, str) and token.startswith(flag + "=") for token in command):
            raise ValueError("frozen finite-query policy and command disagree")
        if index and expected and (positions[0] + 1 >= len(command)
                or command[positions[0] + 1] != str(value)):
            raise ValueError("frozen finite-query allowance and command disagree")


def omit_absent(options):
    """Keep legacy/default-off frozen option dictionaries byte-for-byte stable."""
    for key in KEYS:
        if options.get(key) is None or (key == KEYS[0] and options.get(key) is False):
            options.pop(key, None)
