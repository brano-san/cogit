set -eu
cd "$HOME"
platform=$(uname -sm)
printf "%s\n" "$platform"
[ "$platform" = "Linux x86_64" ] || exit 1
umask 077
d="$HOME/.cache/cogit/agent/$1"
a="$d/cogit-agent"
verified() {
    sum=$(sha256sum < "$1") && [ "${sum%% *}" = "$2" ]
}
command -v sha256sum >/dev/null || { echo "Installing the Cogit agent requires sha256sum." >&2; exit 1; }
if ! { [ -x "$a" ] && verified "$a" "$2"; }; then
    mkdir -p -- "$d"
    t=$(mktemp "$d/.cogit-agent.XXXXXXXX")
    cleanup() { rm -f -- "$t"; }
    trap cleanup EXIT
    trap "exit 1" HUP INT TERM
    printf "upload\n"
    head -c "$3" > "$t"
    verified "$t" "$2" || { echo "The uploaded Cogit agent failed SHA-256 verification." >&2; exit 1; }
    chmod 700 -- "$t"
    mv -fT -- "$t" "$a"
    trap - EXIT HUP INT TERM
fi
printf "ready\n"
export COGIT_INSIDE_AGENT=1 COGIT_SSH_AGENT_SESSION=1 GCM_INTERACTIVE=never GIT_TERMINAL_PROMPT=0
unset SSH_ASKPASS SSH_ASKPASS_REQUIRE COGIT_REPOSITORY_ASKPASS COGIT_ASKPASS_REFUSE COGIT_HOST_KEY_ENDPOINT COGIT_HOST_KEY_TOKEN COGIT_SSH_PASSPHRASE
exec "$a" serve
