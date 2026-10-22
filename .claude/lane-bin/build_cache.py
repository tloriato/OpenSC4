"""Fail closed on an unproved service-owned output path.

Command Job emptiness is not a completion fence for an external cache daemon.
The client-side design still requires authenticated service identity, restart-role
classification and evidence from the actual wrapper dataflow. Until that protocol
exists, never launch a detected cache path and subsequently claim Job drainage.
This is a quarantine, not an implementation of that protocol or a cache-off mode.
"""
from __future__ import annotations

import os
from pathlib import Path
import shlex


class CacheOwnershipError(RuntimeError):
    pass


def require_output_ownership(cmd, env=None):
    """Reject detected wrapper/launcher paths before opening a log or launching.

    Deliberately has no environment-variable bypass or user-supplied proof token.
    This does not discover arbitrary service calls hidden inside a command or a
    Cargo/CMake configuration file; it must not be advertised as whole-worktree
    service containment. Empty wrapper values retain their uncached meaning.
    """
    values = {str(k).upper(): str(v) for k, v in (os.environ if env is None else env).items()}
    args = ([arg.strip('"') for arg in shlex.split(cmd, posix=os.name != "nt")]
            if isinstance(cmd, str) else [os.fspath(arg) for arg in cmd])
    reasons = []
    for key in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER",
                "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"):
        if values.get(key):
            reasons.append(key)
    if args and Path(args[0]).name.lower() in ("sccache", "sccache.exe", "ccache", "ccache.exe"):
        reasons.append("cache executable")
    for arg in args[1:]:
        setting = arg.upper()
        if setting.startswith("-DFC4_COMPILER_CACHE=") and setting.split("=", 1)[1] not in (
                "", "OFF", "0", "FALSE", "NO"):
            reasons.append("FC4_COMPILER_CACHE")
        if setting.startswith(("-DCMAKE_C_COMPILER_LAUNCHER=", "-DCMAKE_CXX_COMPILER_LAUNCHER=")):
            if setting.split("=", 1)[1]:
                reasons.append("CMake compiler launcher")
    if not reasons:
        return
    detail = ""
    if "SCCACHE_LOG" in values:
        detail = " SCCACHE_LOG is present (even empty disables client-side mode in the inspected binary)."
    elif values.get("SCCACHE_CLIENT_SIDE", "").lower() not in ("1", "true", "on"):
        detail = " SCCACHE_CLIENT_SIDE intent alone cannot establish effective client-side execution."
    raise CacheOwnershipError(
        "build/check: service output ownership unverified (" + ", ".join(reasons) + "). "
        "Cached launch rejected before execution: authenticated storage-only service identity, "
        "effective client mode and restart-role evidence are not implemented. "
        "Keep the cache configuration and endpoint; this invocation has no legacy or cache-off fallback."
        + detail + " SCCACHE_ERROR_LOG is a separate setting. "
        "Coordinator action: complete and validate the reviewed service protocol before retrying this cached check.")
