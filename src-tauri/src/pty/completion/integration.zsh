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
typeset -g _vlxc_mode=auto _vlxc_mode_file=${_vlxc_selection:h:h}/mode _vlxc_hint='' _vlxc_hint_style=''
typeset -gA _vlxc_hint_fallback
_vlxc_load_mode() { [[ -r $_vlxc_mode_file ]] && _vlxc_mode=$(<$_vlxc_mode_file) }
_vlxc_hint_clear() {
  [[ -n $_vlxc_hint_style ]] && region_highlight=("${(@)region_highlight:#$_vlxc_hint_style}")
  [[ -n $_vlxc_hint && $POSTDISPLAY == $_vlxc_hint ]] && POSTDISPLAY=''
  _vlxc_hint='' _vlxc_hint_style=''
}
_vlxc_hint_update() {
  setopt localoptions extendedglob
  local match='' hint pattern
  # Stay out of zsh-autosuggestions' way, other POSTDISPLAY owners, typeahead and incremental search.
  if [[ $_vlxc_mode == auto && -n ${BUFFER//[[:space:]]/} && $CURSOR -eq ${#BUFFER} && $KEYMAP != vicmd ]] \
      && (( ! PENDING && ! KEYS_QUEUED_COUNT && ! ${ISEARCHMATCH_ACTIVE:-0} && ! $+functions[_zsh_autosuggest_fetch] )) \
      && [[ -z $POSTDISPLAY || $POSTDISPLAY == $_vlxc_hint ]]; then
    pattern="${BUFFER//(#m)[\\*?[\]<>()|^~#]/\\$MATCH}?*"
    match=${history[(r)$pattern]}
    [[ $match == *$'\n'* ]] && match=''
  fi
  hint=${match#$BUFFER}
  if [[ -z $match || $hint != $_vlxc_hint || $POSTDISPLAY != $_vlxc_hint ]]; then
    _vlxc_hint_clear
    [[ -n $match ]] || return
    _vlxc_hint=$hint POSTDISPLAY=$hint
    _vlxc_hint_style="${#BUFFER} $(( ${#BUFFER} + ${#hint} )) fg=8"
  fi
  # Highlighters may rebuild region_highlight between redraws.
  (( ${region_highlight[(Ie)$_vlxc_hint_style]} )) || region_highlight+=("$_vlxc_hint_style")
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
_vlxc_finish() { _vlxc_hint_clear; zle -R; _vlxc_busy }
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
  [[ $revision == $_vlxc_revision && ( $index == <-> || $index == p ) ]] || return
  [[ $BUFFER == $_vlxc_buffer && $CURSOR == $_vlxc_cursor ]] || return
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
zle -N _vlxc_accept
for _vlxc_map in emacs viins; do
  bindkey -M "$_vlxc_map" '^[[24;5~' _vlxc_query
  bindkey -M "$_vlxc_map" '^[[23;5~' _vlxc_accept
done
unset _vlxc_map
