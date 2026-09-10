# Source this file into sh, bash, or a POSIX-compatible shell. No downloader or JSON parser.
# Each invocation uses a subshell so caller variables, options, traps and cwd stay intact.
saucepan_call() (
    case ${SAUCEPAN_AUTHORITATIVE-} in
        ''|0) ;;
        1) set -- --authoritative "$@" ;;
        *) printf '%s\n' 'SAUCEPAN_AUTHORITATIVE must be 0 or 1' >&2; exit 64 ;;
    esac
    if [ -n "${SAUCEPAN_APP-}" ]; then set -- "--app=$SAUCEPAN_APP" "$@"; fi
    if [ -n "${SAUCEPAN_MARKER-}" ]; then set -- "--marker=$SAUCEPAN_MARKER" "$@"; fi
    if [ "${SAUCEPAN_TEST_ROOT+x}" = x ] || [ "${SAUCEPAN_TEST_KEY+x}" = x ]; then
        if [ -z "${SAUCEPAN_TEST_ROOT-}" ] || [ -z "${SAUCEPAN_TEST_KEY-}" ]; then
            printf '%s\n' 'Test mode requires both SAUCEPAN_TEST_ROOT and SAUCEPAN_TEST_KEY' >&2
            exit 64
        fi
        case $SAUCEPAN_TEST_KEY in
            *[!0-9a-fA-F]*) printf '%s\n' 'Test key must be 64 hexadecimal characters' >&2; exit 64 ;;
        esac
        if [ "${#SAUCEPAN_TEST_KEY}" -ne 64 ]; then
            printf '%s\n' 'Test key must be 64 hexadecimal characters' >&2; exit 64
        fi
        set -- "--test-root=$SAUCEPAN_TEST_ROOT" "--test-key=$SAUCEPAN_TEST_KEY" "$@"
    fi
    if [ -n "${SAUCEPAN_BIN-}" ]; then
        saucepan_binary=$SAUCEPAN_BIN
    else
        : "${HOME:?HOME is required unless SAUCEPAN_BIN is supplied}"
        saucepan_binary=$HOME/.saucepan/bin/saucepan
        if [ ! -x "$saucepan_binary" ] && [ -x "$saucepan_binary.exe" ]; then
            saucepan_binary=$saucepan_binary.exe
        fi
    fi
    "$saucepan_binary" "$@"
)

saucepan_init()              { saucepan_call init "$@"; }
saucepan_register()          { saucepan_call register "$@"; }
saucepan_configure()         { saucepan_call configure "$@"; }
saucepan_acquire()           { saucepan_call acquire "$@"; }
saucepan_view()              { saucepan_call view "$@"; }
saucepan_verify()            { saucepan_call verify "$@"; }
saucepan_path()              { saucepan_call path "$@"; }
saucepan_mirror()            { saucepan_call mirror "$@"; }
saucepan_history()           { saucepan_call history "$@"; }
saucepan_snapshot()          { saucepan_call snapshot "$@"; }
saucepan_shared_executable() { saucepan_call shared-executable "$@"; }
