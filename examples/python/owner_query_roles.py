"""Exact-ID query scope, bound as part of the original query document.

This is input validation, not mathematical coverage authority. Only the Rust
walker and its independent verifier decide whether a required query closes.
"""

import json


def loads_document(text):
    """Reject duplicate keys before JSON decoding can erase the ambiguity."""
    def unique_object(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON field: {key}")
            result[key] = value
        return result
    return json.loads(text, object_pairs_hook=unique_object)


def query_roles(document, require_explicit=False):
    ids = [row["id"] for row in document["queries"]]
    if any(not isinstance(identity, str) for identity in ids) or len(set(ids)) != len(ids):
        raise ValueError("query ids must be unique strings")
    if "query_roles" not in document:
        if require_explicit:
            raise ValueError("rescue requires an explicit complete query_roles declaration")
        return {identity: "required" for identity in ids}
    declaration = document["query_roles"]
    if not isinstance(declaration, dict) or set(declaration) != {"required", "auxiliary"}:
        raise ValueError("query_roles must contain required and auxiliary exact-ID arrays")
    known, roles = set(ids), {}
    for role in ("required", "auxiliary"):
        values = declaration[role]
        if not isinstance(values, list):
            raise ValueError(f"query_roles.{role} must be an exact-ID array")
        for identity in values:
            if not isinstance(identity, str) or identity not in known:
                raise ValueError(f"query_roles names unknown query {identity!r}")
            if identity in roles:
                raise ValueError(f"query_roles repeats query {identity!r}")
            roles[identity] = role
    if set(roles) != known:
        raise ValueError("query_roles must declare every query exactly once")
    return roles
