# YUME Windows paint reentry patch

This directory vendors the published `tao 0.35.3` crate under its Apache-2.0 license.
Registry archive SHA-256:
`d1c93047acf68669466a34690ac58cca7010bd1b201e1ec86f1fd0a75d3dd4a9`.
The archive's `.cargo_vcs_info.json` identifies `5a14e624c81b7a799728129417e9218be25f17d9`
with `dirty: true`; the archive checksum, not a Git checkout, identifies the exact source.

The app uses this copy through `[patch.crates-io]` in `src-tauri/Cargo.toml`.
Runtime dependencies and platform implementations retain the published versions and source.
The local `Cargo.lock` records the standalone regression test's development dependencies;
the application's separate lockfile remains authoritative for application builds.

Changes from the archive:

- Windows `thread_event_target_callback` now completes a redraw cycle only when
  `flush_paint_messages` returns ownership. A synchronous native callback can reenter
  this thread target while another redraw flush owns the cycle. Treating that return
  value as an assertion caused a process panic. The public-window paint handler already
  follows the same ownership rule.
- A Windows-only unit test enters an outer redraw cycle and uses an actual hidden
  Win32 window subclass to send nested `WM_PAINT` to the thread target. It checks no
  panic, exactly one completion, and a subsequent user event and redraw cycle.

Run from the repository root on Windows:

```powershell
$env:RUSTUP_TOOLCHAIN='stable'
cargo test --locked --manifest-path src-tauri/vendor/tao/Cargo.toml --target-dir src-tauri/target --lib
```

The unpatched callback fails with the same `flush_paint_messages` assertion seen in
B7. The patched test passed and was repeated in 25 fresh processes. See
`docs/performance-evidence/b7-windows-tao-sse-20261027` for evidence and limitations.
This proves the reentry mechanism, not which external callback triggered the original
GUI incident. macOS/Linux builds are not validated by this Windows test.

Remove this patch only after a compatible upstream release includes the same ownership
fix and passes this regression plus the normal application gates. Upgrading Tao without
checking the handler is insufficient; the upstream development handler inspected during
this investigation still asserted ownership.
