# VelaTerm native completion. This file is sourced after the user's startup files.
[[ -o interactive ]] || return
typeset -g _vlxc_nonce='@NONCE@' _vlxc_selection='@SELECTION@'
typeset -gi _vlxc_revision=0 _vlxc_cursor=0 _vlxc_capturing=0
typeset -ga _vlxc_values _vlxc_cursors _vlxc_labels _vlxc_descriptions
typeset -g _vlxc_buffer _vlxc_prefix_value
typeset -gi _vlxc_prefix_cursor=0
_vlxc_emit() { printf '\e]6973;%s;%s\a' "$_vlxc_nonce" "$1" }
_vlxc_hex() {
  local LC_ALL=C value=$1 byte
  local -i i
  REPLY=''
  for ((i=1; i<=${#value}; i++)); do
    printf -v byte '%02x' "'${value[i]}"
    REPLY+=$byte
  done
}
_vlxc_ready() { _vlxc_emit P }
_vlxc_busy() { _vlxc_emit X }
autoload -Uz add-zsh-hook add-zle-hook-widget
add-zsh-hook preexec _vlxc_busy
add-zle-hook-widget line-init _vlxc_ready
add-zle-hook-widget line-finish _vlxc_finish
_vlxc_keymap() { if [[ $KEYMAP == vicmd ]]; then _vlxc_busy; else _vlxc_ready; fi }
add-zle-hook-widget keymap-select _vlxc_keymap
add-zle-hook-widget isearch-update _vlxc_busy
add-zle-hook-widget isearch-exit _vlxc_keymap
(( $+functions[compdef] )) || { autoload -Uz compinit; compinit -i; }

# Inline history hint: the most recent history entry extending the line, drawn after the cursor in dim text.
# The application writes the mode to a file shared by all sessions; it is read once per prompt.
typeset -g _vlxc_mode=auto _vlxc_mode_file=${_vlxc_selection:h:h}/mode _vlxc_hint='' _vlxc_hint_style='' _vlxc_undo_value=''
typeset -gi _vlxc_hint_reported=0
typeset -gA _vlxc_hint_fallback
# The last history lookup, so typing on does not rescan the whole history on every key: a line with no match
# keeps having none as it grows, and the newest match stays the newest while it still extends the line. History only
# changes between lines, so each new line starts afresh.
typeset -g _vlxc_hint_miss='' _vlxc_hint_hit='' _vlxc_hint_hit_for=''
_vlxc_load_mode() {
  _vlxc_hint_miss='' _vlxc_hint_hit='' _vlxc_hint_hit_for=''
  [[ -r $_vlxc_mode_file ]] && _vlxc_mode=$(<$_vlxc_mode_file)
}
# zsh shifts region_highlight entries as text is inserted or deleted, so an entry cannot be found again by the text
# it was added with. zsh 5.9 keeps a memo= tag on each entry; older versions fall back to every plain dim range.
typeset -g _vlxc_hint_memo='' _vlxc_hint_entries='<-> <-> fg=8'
autoload -Uz is-at-least
is-at-least 5.9 && _vlxc_hint_memo=' memo=vlxc-hint' _vlxc_hint_entries='* memo=vlxc-hint'
_vlxc_hint_clear() {
  [[ -n $_vlxc_hint_style ]] && region_highlight=("${(@)region_highlight:#${~_vlxc_hint_entries}}")
  [[ -n $_vlxc_hint && $POSTDISPLAY == $_vlxc_hint ]] && POSTDISPLAY=''
  _vlxc_hint='' _vlxc_hint_style=''
}
# Tells the terminal view whether a hint is on screen so its header can name the key. The sequence has no width and,
# unlike the nonce protocol, carries nothing a forgery could abuse: it only drives that label.
_vlxc_hint_report() {
  local -i shown=$(( ${#_vlxc_hint} > 0 ))
  (( shown == _vlxc_hint_reported )) && return
  _vlxc_hint_reported=$shown
  printf '\e]6974;%d\a' $shown
}
_vlxc_hint_update() {
  setopt localoptions extendedglob
  local match='' hint pattern
  # Stay out of zsh-autosuggestions' way, other POSTDISPLAY owners, typeahead and incremental search.
  if [[ $_vlxc_mode == auto && -n ${BUFFER//[[:space:]]/} && $CURSOR -eq ${#BUFFER} && $KEYMAP != vicmd ]] \
      && (( ! PENDING && ! KEYS_QUEUED_COUNT && ! ${ISEARCHMATCH_ACTIVE:-0} && ! $+functions[_zsh_autosuggest_fetch] )) \
      && [[ -z $POSTDISPLAY || $POSTDISPLAY == $_vlxc_hint ]]; then
    if [[ -n $_vlxc_hint_miss && $BUFFER == ${_vlxc_hint_miss}* ]]; then
      match=''
    elif [[ -n $_vlxc_hint_hit && $BUFFER == ${_vlxc_hint_hit_for}* && $_vlxc_hint_hit == ${BUFFER}?* ]]; then
      match=$_vlxc_hint_hit
    else
      pattern="${BUFFER//(#m)[\\*?[\]<>()|^~#]/\\$MATCH}?*"
      match=${history[(r)$pattern]}
      if [[ -z $match ]]; then
        _vlxc_hint_miss=$BUFFER
      elif [[ $match == *$'\n'* ]]; then
        # An older single-line entry may still match a longer line, so this is no reason to stop looking.
        match=''
      else
        _vlxc_hint_hit=$match _vlxc_hint_hit_for=$BUFFER
      fi
    fi
  fi
  hint=${match#$BUFFER}
  # Rebuild the range on every redraw: an edit may have moved the old one onto typed text, and highlighters may
  # rebuild region_highlight between redraws.
  _vlxc_hint_clear
  [[ -n $match ]] || { _vlxc_hint_report; return }
  _vlxc_hint=$hint POSTDISPLAY=$hint
  _vlxc_hint_style="${#BUFFER} $(( ${#BUFFER} + ${#hint} )) fg=8$_vlxc_hint_memo"
  region_highlight+=("$_vlxc_hint_style")
  _vlxc_hint_report
}
# Right arrow, End and Ctrl+E take the hint at the end of the line and otherwise run the widget they had.
_vlxc_hint_accept() {
  if [[ -n $_vlxc_hint && $POSTDISPLAY == $_vlxc_hint && $CURSOR -eq ${#BUFFER} ]]; then
    local hint=$_vlxc_hint
    _vlxc_hint_clear
    BUFFER+=$hint CURSOR=${#BUFFER}
    return
  fi
  local keymap=$KEYMAP
  [[ $keymap == main ]] && keymap=$_vlxc_main_keymap
  zle ${_vlxc_hint_fallback[$keymap:$KEYS]:-forward-char} -w
}
_vlxc_finish() { _vlxc_hint_clear; _vlxc_hint_report; zle -R; _vlxc_busy }
zle -N _vlxc_hint_accept
add-zle-hook-widget line-pre-redraw _vlxc_hint_update
add-zle-hook-widget line-init _vlxc_load_mode
typeset -g _vlxc_main_keymap=emacs
[[ $(bindkey -lL main) == *viins* ]] && _vlxc_main_keymap=viins
() {
  local keymap key binding
  for keymap in emacs viins; do
    for key in $'\e[C' $'\eOC' $'\e[F' $'\eOF' $'\x05'; do
      [[ $keymap == viins && $key == $'\x05' ]] && continue
      binding=${${(z)"$(bindkey -M $keymap -- $key)"}[2]}
      [[ $binding == _vlxc_hint_accept ]] && continue
      if [[ -z $binding || $binding == undefined-key ]]; then
        if [[ $key == $'\e'?C ]]; then binding=forward-char; else binding=end-of-line; fi
      fi
      _vlxc_hint_fallback[$keymap:$key]=$binding
      bindkey -M $keymap -- $key _vlxc_hint_accept
    done
  done
}

_vlxc_compadd() {
  local -a _vlxc_hits _vlxc_display
  local -i hit_index=0 file_matches=0
  local arg hit left right replacement suffix='' prefix='' pending='' display_name='' file_prefix='' display_label extra_suffix
  # Capture-only requests belong to completers themselves, not the displayed candidate list.
  for arg in "$@"; do
    if [[ $arg == -[OAD]* ]]; then builtin compadd "$@"; return; fi
    if [[ -n $pending ]]; then
      case $pending in -S|-s) suffix+=$arg;; -P|-p) prefix+=$arg;; -d) display_name=$arg;; -W) file_prefix=$arg;; esac
      pending=''
    elif [[ $arg == -- ]]; then break
    elif [[ $arg == -[SPsp] ]]; then pending=$arg
    elif [[ $arg == -d || $arg == -ld ]]; then pending=-d
    elif [[ $arg == -W ]]; then pending=-W
    elif [[ $arg =~ '^-[a-zA-Z]*f[a-zA-Z]*$' ]]; then file_matches=1
    fi
  done
  if [[ $display_name == \(*\) ]]; then
    local display_words=${display_name[2,-2]}
    _vlxc_display=("${(@Q)${(z)display_words}}")
  elif [[ -n $display_name ]]; then
    _vlxc_display=("${(@P)display_name}")
  fi
  builtin compadd -A _vlxc_hits -D _vlxc_display "$@"
  # Preserve native match accounting so fallback matchers do not add unrelated candidates.
  builtin compadd "$@"
  left=${BUFFER[1,$(( CURSOR - ${#PREFIX} ))]}
  right=${BUFFER[$(( CURSOR + ${#SUFFIX} + 1 )),-1]}
  for hit in "${_vlxc_hits[@]}"; do
    (( ++hit_index ))
    (( ${#_vlxc_values} >= 64 )) && break
    display_label=${(Q)hit}
    extra_suffix=$suffix
    if (( file_matches )) && [[ -d "${(Q)file_prefix}${(Q)hit}" && -z $extra_suffix ]]; then
      [[ $hit == */ ]] || extra_suffix=/
      [[ $display_label == */ ]] || display_label+=/
    fi
    [[ ${_vlxc_labels[(Ie)$display_label]} != 0 ]] && continue
    replacement="${prefix}${hit}${extra_suffix}"
    _vlxc_values+=("${left}${replacement}${right}")
    _vlxc_cursors+=($(( ${#left} + ${#replacement} )))
    _vlxc_labels+=("$display_label")
    _vlxc_descriptions+=("${_vlxc_display[hit_index]-}")
  done
}
_vlxc_complete_internal() {
  local REPLY
  _vlxc_hex "${(Q)PREFIX}"
  _vlxc_emit "T;$REPLY"
  local previous=${functions[compadd]-}
  functions[compadd]=$functions[_vlxc_compadd]
  { _main_complete 2>/dev/null } always {
    if [[ -n $previous ]]; then functions[compadd]=$previous; else unfunction compadd; fi
    compstate[insert]=''
    compstate[list]=''
  }
}
zle -C _vlxc_collect complete-word _vlxc_complete_internal
_vlxc_query() {
  local saved_status=$? label label_hex REPLY
  local -i item=0
  # Tab with the "accept" behavior: a visible hint wins over completion, and the typed text is kept for the undo.
  if [[ $WIDGET == _vlxc_query_tab && -n $_vlxc_hint && $POSTDISPLAY == $_vlxc_hint && $CURSOR -eq ${#BUFFER} ]]; then
    _vlxc_undo_value=$BUFFER
    local hint=$_vlxc_hint
    _vlxc_hint_clear
    BUFFER+=$hint CURSOR=${#BUFFER}
    _vlxc_buffer=$BUFFER _vlxc_cursor=$CURSOR
    _vlxc_values=() _vlxc_prefix_value=''
    (( ++_vlxc_revision ))
    _vlxc_emit "A;$_vlxc_revision"
    _vlxc_emit Y
    _vlxc_emit Z
    zle -R
    return $saved_status
  fi
  _vlxc_undo_value=''
  _vlxc_buffer=$BUFFER _vlxc_cursor=$CURSOR
  _vlxc_values=() _vlxc_cursors=() _vlxc_labels=() _vlxc_descriptions=()
  (( ++_vlxc_revision ))
  _vlxc_emit "A;$_vlxc_revision"
  if [[ -n $BUFFER ]]; then
    zle _vlxc_collect
  fi
  BUFFER=$_vlxc_buffer CURSOR=$_vlxc_cursor
  _vlxc_prefix
  for label in "${_vlxc_labels[@]}"; do
    (( ++item ))
    _vlxc_hex "$label"; label_hex=$REPLY
    _vlxc_hex "${_vlxc_descriptions[item]}"
    _vlxc_emit "C;$label_hex;$REPLY"
  done
  _vlxc_emit Z
  zle -R
  return $saved_status
}
# Like native Tab, offer the text all candidates share when it extends what was typed.
_vlxc_prefix() {
  local common head tail
  local -i i
  _vlxc_prefix_value='' _vlxc_prefix_cursor=0
  (( ${#_vlxc_values} > 1 )) || return
  common=${_vlxc_values[1][1,${_vlxc_cursors[1]}]}
  for ((i=2; i<=${#_vlxc_values}; i++)); do
    head=${_vlxc_values[i][1,${_vlxc_cursors[i]}]}
    while [[ -n $common && ${head[1,${#common}]} != "$common" ]]; do common=${common[1,-2]}; done
  done
  # Never stop inside an escape sequence.
  tail=${common##*[^\\]}
  (( ${#tail} % 2 )) && common=${common[1,-2]}
  (( ${#common} > _vlxc_cursor )) && [[ ${common[1,_vlxc_cursor]} == "${_vlxc_buffer[1,_vlxc_cursor]}" ]] || return
  _vlxc_prefix_value=$common${_vlxc_values[1][$(( ${_vlxc_cursors[1]} + 1 )),-1]}
  _vlxc_prefix_cursor=${#common}
  _vlxc_emit E
}
_vlxc_accept() {
  local revision index
  read -r revision index < "$_vlxc_selection" || return
  [[ $revision == $_vlxc_revision && ( $index == <-> || $index == p || $index == u ) ]] || return
  [[ $BUFFER == $_vlxc_buffer && $CURSOR == $_vlxc_cursor ]] || return
  if [[ $index == u ]]; then
    [[ -n $_vlxc_undo_value ]] || return
    BUFFER=$_vlxc_undo_value CURSOR=${#BUFFER}
    _vlxc_undo_value=''
    zle -R
    return
  fi
  if [[ $index == p ]]; then
    [[ -n $_vlxc_prefix_value ]] || return
    BUFFER=$_vlxc_prefix_value CURSOR=$_vlxc_prefix_cursor
    _vlxc_values=() _vlxc_prefix_value=''
    zle -R
    return
  fi
  (( index >= 0 && index < ${#_vlxc_values} )) || return
  BUFFER=${_vlxc_values[$((index + 1))]}
  CURSOR=${_vlxc_cursors[$((index + 1))]}
  _vlxc_values=()
  zle -R
}
zle -N _vlxc_query
zle -N _vlxc_query_tab _vlxc_query
zle -N _vlxc_accept
for _vlxc_map in emacs viins; do
  bindkey -M "$_vlxc_map" '^[[24;5~' _vlxc_query
  bindkey -M "$_vlxc_map" '^[[21;5~' _vlxc_query_tab
  bindkey -M "$_vlxc_map" '^[[23;5~' _vlxc_accept
done
unset _vlxc_map
# Lets the backend send the Tab key that can accept a hint; other shells never declare it.
_vlxc_emit H
