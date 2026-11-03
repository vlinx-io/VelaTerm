status is-interactive; or return
set -g _vlxc_nonce '@NONCE@'
set -g _vlxc_selection '@SELECTION@'
set -g _vlxc_revision 0
function _vlxc_emit
    printf '\e]6973;%s;%s\a' $_vlxc_nonce $argv[1]
end
function _vlxc_hex
    env LC_ALL=C od -An -v -tx1 | string replace -ar '[ \n]' '' | string join ''
end
function _vlxc_ready --on-event fish_prompt
    _vlxc_emit P
end
function _vlxc_busy --on-event fish_preexec
    _vlxc_emit X
end
function _vlxc_query
    set -g _vlxc_buffer (commandline --current-buffer | string collect)
    set -g _vlxc_cursor (commandline --cursor)
    set -g _vlxc_revision (math $_vlxc_revision + 1)
    set -g _vlxc_values
    set -g _vlxc_prefix ''
    _vlxc_emit "A;$_vlxc_revision"
    set -l query (commandline --current-token --cut-at-cursor | string collect)
    set -l query_hex (printf %s "$query" | _vlxc_hex)
    _vlxc_emit "T;$query_hex"
    for entry in (complete -C (commandline --cut-at-cursor | string collect))
        test (count $_vlxc_values) -ge 64; and break
        set -l parts (string split -m 1 \t -- $entry)
        set -ga _vlxc_values (string escape -- $parts[1])
        set -l label (printf %s $parts[1] | _vlxc_hex)
        set -l description ''
        if test (count $parts) -gt 1
            set description (printf %s $parts[2] | _vlxc_hex)
        end
        _vlxc_emit "C;$label;$description"
    end
    _vlxc_common_prefix "$query"
    _vlxc_emit Z
    commandline -f repaint
end
# Like native Tab, offer the text all candidates share when it extends the token typed so far.
function _vlxc_common_prefix
    test (count $_vlxc_values) -gt 1; or return
    set -l common $_vlxc_values[1]
    for value in $_vlxc_values[2..-1]
        while test -n "$common"
            set -l head (string sub -l (string length -- "$common") -- "$value")
            test "$head" = "$common"; and break
            set common (string sub -l (math (string length -- "$common") - 1) -- "$common")
        end
    end
    # Never stop inside an escape sequence.
    string match -qr '(?<!\\\\)(?:\\\\\\\\)*\\\\$' -- "$common"; and set common (string sub -l (math (string length -- "$common") - 1) -- "$common")
    set -l typed (string length -- "$argv[1]")
    test (string length -- "$common") -gt $typed; or return
    set -l head (string sub -l $typed -- "$common")
    test "$head" = "$argv[1]"; or return
    set -g _vlxc_prefix $common
    _vlxc_emit E
end
function _vlxc_accept
    read -l revision index < $_vlxc_selection; or return
    test "$revision" = "$_vlxc_revision"; or return
    string match -qr '^([0-9]+|p)$' -- "$index"; or return
    test (commandline --cursor) = $_vlxc_cursor; or return
    test (commandline --current-buffer | string collect) = "$_vlxc_buffer"; or return
    if test "$index" = p
        test -n "$_vlxc_prefix"; or return
        commandline --current-token --replace -- $_vlxc_prefix
        set -g _vlxc_values
        set -g _vlxc_prefix ''
        commandline -f repaint
        return
    end
    set -l slot (math $index + 1)
    test $slot -le (count $_vlxc_values); or return
    commandline --current-token --replace -- $_vlxc_values[$slot]
    set -g _vlxc_values
    commandline -f repaint
end
bind \e\[24\;5~ _vlxc_query
bind \e\[23\;5~ _vlxc_accept
if bind --list-modes | string match -q insert
    bind -M insert \e\[24\;5~ _vlxc_query
    bind -M insert \e\[23\;5~ _vlxc_accept
end
