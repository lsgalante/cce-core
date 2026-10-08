# CLAUDE.md

This is the `cce-core` crate, the GUI-free half of the cce toolkit. Read the
workspace guide `../cce-compositor/WORKSPACE.md` first: the multi-repo layout,
the standalone-build rule, `ccebuild`, and the concurrent-sessions rules. This
crate is SHARED like `cce-ui`. Run `git status` before editing it, and treat
foreign dirt as another session's.

## What lives here

| Module | What it is |
|---|---|
| `config` | the KDL config: paths (`~/.config/cce`, XDG), reading, writing with rolling backups, the JSON view |
| `input` | `input.kdl`: domain-scoped bindings and pointer settings |
| `motion` | the DE-wide animations switch (`/run/cce/animations`) |
| `units` | lengths with units (`(mm)2.0`) and the display metric |
| `ipc` | the `/tmp/<prefix>-<WAYLAND_DISPLAY>.sock` convention, `ipc::instance` (not wasm) |
| `color`, `ramp`, `relief_spec`, `droplet` | the parsers for the specs the DE writes: hex colours, ramp curves, relief, droplets |

`cce-ui` re-exports every module at its old path (`cce_ui::config`,
`cce_ui::motion`, `cce_ui::scene::paint::DropletSpec`, `cce_ui::layout::sample_ramp_keys`,
…), so apps never name this crate. A process that does not draw (the compositor,
a sync daemon, a CLI helper) depends on it directly and links none of the
toolkit's Wayland, Vulkan or text stack. Nothing here may depend on cce-ui; that
is the point of the split.

## The test-isolation feature

Several functions answer differently under `cfg(test)`, so a suite never reads
the machine. `config::config_home()` is a per-process directory nobody creates,
`input::natural_scroll()` reads false, and `motion::enabled()` is on unless
`motion::force_for_test` says otherwise. Since those modules live here,
`cfg(test)` would only cover this crate's own suite. Every such gate is
`cfg(any(test, feature = "test-isolation"))` instead, and cce-ui turns the
feature on through its dev-dependency. Resolver 2 keeps it out of normal builds,
and a dependent's test build that does not build cce-ui's dev-dependencies does
not get it either, which is what those builds had before the split.

## Build, test

```sh
cargo test -p cce-core
cargo test -p cce-core --features test-isolation
```

The crate builds for `wasm32-unknown-unknown` too (minus `ipc`), as cce-ui does.
After pushing, run `../bump-revs.sh cce-core` to repin cce-ui and the compositor.
