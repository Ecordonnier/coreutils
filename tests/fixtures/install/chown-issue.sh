#!/bin/bash
# Regression test for install chown issue #9116
# Tests that install works under pseudo with PSEUDO_IGNORE_PATHS
# (simulates Yocto/OpenEmbedded build environment)

set -e

# Pseudo availability is checked by the Rust test
# Set up test environment
TEST_DIR=$(mktemp -d)
trap "rm -rf $TEST_DIR" EXIT

cd "$TEST_DIR"

# Set up pseudo environment using LD_PRELOAD approach  
# This is needed to reproduce the specific failure scenario
mkdir -p testenv/bin testenv/var/pseudo
ln -s /usr/bin/pseudo testenv/bin/pseudo
export LD_PRELOAD="/usr/lib/x86_64-linux-gnu/pseudo/libpseudo.so"
export PSEUDO_PREFIX="$TEST_DIR/testenv"
export PSEUDO_IGNORE_PATHS="$TEST_DIR"

# Check if we appear to be root (pseudo working)
current_uid=$(timeout 5s id -u 2>/dev/null || echo "failed")

if [ "$current_uid" != "0" ]; then
    echo "SKIP: pseudo not working - not running as simulated root (id -u = $current_uid)" >&2
    echo "SKIP: libpseudo.so may not be installed or functional" >&2
    exit 2  # Special exit code for "skip test"
fi

echo "Pseudo working: simulated root mode active (id -u = $current_uid)" >&2

# Get the install binary path from first argument, or use 'install' from PATH
INSTALL_BIN="${1:-install}"

echo "Testing install under pseudo environment..." >&2
echo "Using install binary: $INSTALL_BIN" >&2

# Handle both multicall binary and individual install binary
if [[ "$INSTALL_BIN" == */coreutils ]]; then
    # Multicall binary - call with 'install' subcommand
    INSTALL_CMD=("$INSTALL_BIN" install)
else
    # Individual install binary
    INSTALL_CMD=("$INSTALL_BIN")
fi

# This should work without chown errors with the fix
# (but FAIL with explicit chown calls due to PSEUDO_IGNORE_PATHS)
if "${INSTALL_CMD[@]}" -d -m 755 some/directory/to/create 2>install_stderr.log; then
    # Check that no chown errors occurred
    if grep -q "failed to chown" install_stderr.log; then
        echo "FAIL: install called chown and failed under pseudo" >&2
        echo "Error output:" >&2
        cat install_stderr.log >&2
        exit 1
    else
        echo "PASS: install worked without chown errors" >&2
        # Verify directory was actually created
        if [ -d "some/directory/to/create" ]; then
            echo "PASS: directory was created successfully" >&2
            exit 0
        else
            echo "FAIL: directory was not created" >&2
            exit 1
        fi
    fi
else
    echo "FAIL: install command failed" >&2
    echo "Error output:" >&2
    cat install_stderr.log >&2
    exit 1
fi
