#!/usr/bin/env fish
#
# Launcher for aliens-vs-suburbia.
#
#   ./run.fish run_dev [-- cargo/game args...]   fastest iteration build
#   ./run.fish run_rel [-- cargo/game args...]   optimised release build
#
# run_dev mirrors the `run` job in bacon.toml: nightly toolchain, cranelift as
# the dev codegen backend, sccache in front of rustc, the parallel rustc frontend
# (`-Z threads=8`) and `hint-mostly-unused` for dependencies only. Cranelift
# compiles much faster than LLVM but produces slower code, so it is dev-only.
# The env and flags match bacon exactly, so the two share sccache entries and
# don't invalidate each other's build artefacts in target/.
#
# run_rel mirrors the `run-release` job in bacon.toml: the same nightly env and
# flags, plus --release. The cranelift env var only touches the dev profile, so
# the release profile in Cargo.toml (LLVM, thin LTO) still applies and the binary
# is actually fast at runtime; it still gets sccache and the parallel frontend.
#
# Extra arguments are appended to the cargo invocation. Anything after a `--`
# separator is passed through to the game itself, e.g.
#   ./run.fish run_dev --features map-editor
#   ./run.fish run_rel -- --some-game-flag

set -l script_dir (dirname (status --current-filename))
cd $script_dir; or exit 1

set -l mode $argv[1]
set -l extra $argv[2..-1]

# Number of codegen jobs. Override with e.g. `JOBS=4 ./run.fish run_dev`.
set -q JOBS; or set -l JOBS (nproc)

function __usage
    echo "usage: ./run.fish {run_dev|run_rel} [extra cargo args...]"
    echo
    echo "  run_dev  nightly + cranelift + sccache + parallel frontend, dev profile (fastest build)"
    echo "  run_rel  nightly + sccache + parallel frontend, --release (fastest runtime)"
end

function __require
    if not command -q $argv[1]
        echo "run.fish: '$argv[1]' not found on PATH." >&2
        echo "run.fish: $argv[2]" >&2
        return 1
    end
end

function __require_nightly
    if not rustup toolchain list | string match -qr '^nightly-'
        echo "run.fish: no nightly toolchain installed." >&2
        echo "run.fish: install it with: rustup toolchain install nightly" >&2
        return 1
    end
end

# Shared by both modes, identical to bacon.toml's [env] and job flags so bacon
# and this script reuse each other's artefacts instead of rebuilding.
function __cargo_nightly
    env CARGO_TERM_COLOR=always \
        RUSTC_WRAPPER=sccache \
        CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift \
        RUSTFLAGS="-Z threads=8" \
        cargo +nightly $argv[1] \
        -Z codegen-backend -Z profile-hint-mostly-unused --timings \
        --config 'profile.dev.package."*".hint-mostly-unused=true' \
        $argv[2..-1]
end

switch $mode
    case run_dev
        __require sccache "install it with: cargo install sccache"; or exit 1
        __require_nightly; or exit 1

        if not rustup component list --toolchain nightly \
                | string match -qr 'rustc-codegen-cranelift.*\(installed\)'
            echo "run.fish: the cranelift codegen backend is not installed for nightly." >&2
            echo "run.fish: install it with:" >&2
            echo "run.fish:   rustup component add rustc-codegen-cranelift-preview --toolchain nightly" >&2
            exit 1
        end

        echo "==> dev build: nightly + cranelift + sccache + -Z threads=8 (-j $JOBS)"
        __cargo_nightly run -j $JOBS $extra

    case run_rel
        __require sccache "install it with: cargo install sccache"; or exit 1
        __require_nightly; or exit 1

        echo "==> release build: nightly + sccache + -Z threads=8 (-j $JOBS)"
        __cargo_nightly run --release -j $JOBS $extra

    case '' -h --help help
        __usage
        test -z "$mode"; and exit 1
        exit 0

    case '*'
        echo "run.fish: unknown mode '$mode'" >&2
        echo >&2
        __usage >&2
        exit 1
end
