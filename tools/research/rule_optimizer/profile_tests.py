"""Boundary and semantic regressions for the bounded research decoder."""
import json
import struct
import unittest

import profile_records as p


def uint(value):
    if value < 251:
        return bytes([value])
    for tag, width in ((251, 2), (252, 4), (253, 8)):
        if value < 1 << (8 * width):
            return bytes([tag]) + value.to_bytes(width, "little")
    raise ValueError("integer overflow")


def blob(value):
    return uint(len(value)) + value


def fixture(native=True, route=False, stats=None):
    # Two coordinates: owner10, lower[0,3], upper[9,infinity], rank7,
    # A<=11, -3<=D<=8. Fields deliberately have distinguishable values.
    image = (bytes([4, route, 2, 1, 0, 2, 0, 3, 2, 9]) + uint(65535)
             + bytes([1, 7, 1, 11, 1, 5, 1, 16, 2]))
    if native:
        # Native(class0,kindApply/Route,v0=10,distinct_edges3,no self/error).
        authority = image + bytes([1, 0, 2 if route else 0, 10, 3, 0, 0, 0, 0, 0,
                                   17, 16, 15, 0, 0, 2, 4,
                                   1, 12, 5, 6, 7, 8, 9, 10, 0])
        diagnostic = struct.pack("<d", 0.25) + blob(json.dumps(stats or {"events": 15}).encode()) + b"\0\0\0\0"
    else:
        authority = image + bytes([0, 3, 0])
        diagnostic = b"\0" * 13
    return struct.pack("<4sII", p.MAGIC, len(authority), len(diagnostic)) + authority + diagnostic


class DecoderTests(unittest.TestCase):
    def test_plan_cannot_bypass_read_bounds(self):
        with self.assertRaises(ValueError):
            p.validate_plan({"schema": "unknown"})
        plan = {"schema": "rustred-profile-window-plan-v1", "windows": [
            {"start": 0, "bytes": (1 << 20) + 1}], "sources": [], "total_requested_bytes": 1}
        with self.assertRaises(ValueError):
            p.validate_plan(plan)

    def test_integer_width_and_signed_extremes(self):
        for value in (0, 250, 251, 65535, 65536, 2**32 - 1, 2**32, 2**64 - 1):
            d = p.Decoder(uint(value))
            self.assertEqual(d.uint(), value)
            d.finish()
        self.assertEqual(p.Decoder(uint(2**64 - 1)).sint(), -(2**63))
        self.assertEqual(p.Decoder(uint(2**64 - 2)).sint(), 2**63 - 1)
        with self.assertRaises(ValueError):
            p.Decoder(b"\xfc\x01\0\0\0").uint(16)

    def test_native_semantic_fields_remain_distinct(self):
        frame = fixture()
        r, end = p.decode_frame(frame, 0)
        self.assertEqual(end, len(frame))
        self.assertEqual((r["id"], r["phase"], r["owner"]), (4, "Apply", "10"))
        self.assertEqual(r["lower"], [0, 3])
        self.assertEqual(r["upper"], [9, None])
        self.assertEqual(r["power_bounds"], dict(max_positive_power=11, min_power_difference=-3, max_power_difference=8))
        self.assertEqual([r["native"][k] for k in p.COUNTERS], [17, 16, 3, 4, 2])
        self.assertEqual(r["resolver"]["optional"], [6, 7, 8])
        self.assertEqual(r["seconds"], 0.25)
        self.assertEqual(p.decode_frame(fixture(route=True), 0)[0]["phase"], "Route")

    def test_alias_is_not_native(self):
        r, _ = p.decode_frame(fixture(native=False), 0)
        self.assertEqual(r["record_kind"], "delegated_not_inspected")
        self.assertNotIn("native", r)
        self.assertEqual(r["representative_id"], 3)

    def test_synchronization_ignores_false_magic_requires_three_frames(self):
        junk = b"fragmentERB1\0\0\0\0\0\0\0\0junk"
        frame = fixture(stats={"detail": "ERB1 in diagnostic payload"})
        self.assertEqual(p.synchronize(junk + frame * 3 + frame[:20]), len(junk))
        with self.assertRaises(ValueError):
            p.synchronize(junk + frame * 2)

    def test_truncation_and_corruption_fail_closed(self):
        frame = fixture()
        with self.assertRaises(EOFError):
            p.decode_frame(frame[:-1], 0)
        with self.assertRaises(ValueError):
            p.decode_frame(b"NOPE" + frame[4:], 0)
        with self.assertRaises(ValueError):
            p.Decoder(b"\2").boolean()
        with self.assertRaises(ValueError):
            p.decode_authority(frame[12:12 + struct.unpack_from("<I", frame, 4)[0]] + b"\0")


if __name__ == "__main__":
    unittest.main()
