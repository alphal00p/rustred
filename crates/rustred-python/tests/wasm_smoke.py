"""Run inside Pyodide after installing the matching RustRed or HEPKit wheel.

Standalone: exec(open("wasm_smoke.py").read()); run_smoke()
HEPKit:    run_smoke(hep.rustred)

The same smoke is useful on native Python, but only WASM requires synchronous
sessions. No subprocess, thread, FORM executable, or persistent filesystem is
used. All exact algebra and artifact replay remain in the Rust application.
"""


def run_smoke(api=None):
    if api is None:
        import rustred as api
    import sys

    capabilities = api.execution_capabilities()
    wasm = sys.platform in {"emscripten", "wasi"}
    assert capabilities["background_sessions"] is not wasm
    assert capabilities["live_event_polling"] is not wasm
    assert capabilities["max_workers"] == (1 if wasm else None)

    source = """I(name(wasm_tadpole), loops(k), externals(), dimension(d),
        prop(D1,k^2-1,1))"""
    if wasm:
        try:
            api.family_candidates(source, n_cores=2)
        except api.RustRedInputError:
            pass
        else:
            raise AssertionError("WASM must reject unavailable worker counts")

    session = api.start_family_candidates(source, n_cores=1, event_capacity=4)
    assert session.execution_mode == capabilities["execution_mode"]
    if wasm:
        assert session.done, "WASM start is synchronous, not pretend async"
    assert session.wait(timeout=60.0)
    event_batch = session.poll_events(max_events=4, timeout=0.0)
    assert len(event_batch["events"]) <= 4
    assert event_batch["snapshot"]["done"]
    generated = session.result()
    assert isinstance(generated.bundle, bytes)
    # A new reader uses the same binary format as a native build.
    reopened = api.CandidateArtifact.open(generated.bundle)
    assert reopened.metadata()
    certified = api.certify_candidates(generated.bundle)
    assert isinstance(certified.artifact, bytes)
    assert api.inspect_closing_artifact(certified.artifact)
    reduced = api.reduce_with_closing_artifact(certified.artifact, [3])
    assert len(reduced.terms) == 1
    assert list(reduced.terms[0].master_powers) == [1]
    assert reduced.terms[0].common_mass_squared_power == -2
    return {
        "capabilities": capabilities,
        "candidate_bytes": len(generated.bundle),
        "artifact_bytes": len(certified.artifact),
        "master_powers": list(reduced.terms[0].master_powers),
        "common_mass_squared_power": reduced.terms[0].common_mass_squared_power,
        "status": "passed",
    }


if __name__ == "__main__":
    print(run_smoke())
