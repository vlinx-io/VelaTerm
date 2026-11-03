# VelaTerm keeps Readline authoritative and calls the session's registered completion functions.
[[ $- == *i* ]] || return
_vlxc_nonce='@NONCE@'
_vlxc_emit() { printf '\e]6973;%s;%s\a' "$_vlxc_nonce" "$1"; }
# Bash 3 has no READLINE_LINE/READLINE_POINT interface for bind -x callbacks.
if (( BASH_VERSINFO[0] < 4 )); then _vlxc_emit U; return; fi
_vlxc_selection='@SELECTION@'
_vlxc_revision=0
_vlxc_values=() _vlxc_cursors=() _vlxc_prefix_value='' _vlxc_prefix_cursor=0
_vlxc_hex() {
  local LC_ALL=C value=$1 byte i
  REPLY=''
  for ((i=0; i<${#value}; i++)); do
    printf -v byte '%02x' "'${value:i:1}"
    REPLY+=$byte
  done
}
_vlxc_ready() { _vlxc_emit P; }
_vlxc_busy() { _vlxc_emit X; }

_vlxc_query() {
  local saved_status=$? line=$READLINE_LINE point=$READLINE_POINT
  local quoting_locale=${LC_ALL:-${LC_CTYPE:-${LANG:-C}}}
  local LC_ALL=C
  local i ch quote='' escape=0 start=0 end word='' spec func='' prev='' current='' candidate quoted path tilde suffix REPLY
  local -a words=() COMPREPLY=()
  _vlxc_buffer=$line _vlxc_cursor=$point
  _vlxc_values=() _vlxc_cursors=()
  _vlxc_revision=$((_vlxc_revision + 1))
  _vlxc_emit "A;$_vlxc_revision"
  if [[ -z $line ]]; then _vlxc_emit Z; return "$saved_status"; fi
  # Tokenize without evaluating command substitutions or expanding user input.
  for ((i=0; i<point; i++)); do
    ch=${line:i:1}
    if ((escape)); then word+=$ch; escape=0
    elif [[ $ch == \\ && $quote != "'" ]]; then escape=1
    elif [[ -n $quote ]]; then
      if [[ $ch == "$quote" ]]; then quote=''; else word+=$ch; fi
    elif [[ $ch == '"' || $ch == "'" ]]; then quote=$ch
    elif [[ $ch == ' ' || $ch == $'\t' ]]; then
      if ((i > start)); then words+=("$word"); fi
      word='' start=$((i+1))
    elif [[ $ch == ';' || $ch == '|' || $ch == '&' || $ch == $'\n' ]]; then
      words=(); word='' start=$((i+1))
    else word+=$ch
    fi
  done
  current=$word
  _vlxc_hex "$current"
  _vlxc_emit "T;$REPLY"
  words+=("$current")
  end=$point
  while ((end < ${#line})); do
    ch=${line:end:1}
    [[ -z $quote && ( $ch == ' ' || $ch == $'\t' || $ch == ';' || $ch == '|' || $ch == '&' ) ]] && break
    if [[ -n $quote && $ch == "$quote" ]]; then quote=''; fi
    end=$((end+1))
  done
  local COMP_LINE=$line COMP_POINT=$point COMP_CWORD=$((${#words[@]} - 1)) COMP_TYPE=9 COMP_KEY=9
  local -a COMP_WORDS=("${words[@]}")
  if (( COMP_CWORD > 0 )); then
    prev=${words[COMP_CWORD-1]}
    spec=$(builtin complete -p -- "${words[0]}" 2>/dev/null)
    if [[ -z $spec ]]; then
      if declare -F _completion_loader >/dev/null; then _completion_loader "${words[0]}" >/dev/null 2>&1; fi
      spec=$(builtin complete -p -- "${words[0]}" 2>/dev/null)
    fi
    local -a parts=()
    # Only parse Bash's own shell-quoted serialization, never the user's command line.
    if [[ $spec == 'complete '* ]]; then eval "parts=(${spec#complete })"; fi
    for ((i=0; i<${#parts[@]}-1; i++)); do
      if [[ ${parts[i]} == -F ]]; then func=${parts[i+1]}; break; fi
    done
    if [[ $func =~ ^[a-zA-Z0-9_.:-]+$ ]] && declare -F "$func" >/dev/null; then
      "$func" "${words[0]}" "$current" "$prev" >/dev/null 2>&1
    elif ((${#parts[@]} > 1)); then
      while IFS= read -r candidate; do COMPREPLY+=("$candidate"); done < <(
        builtin compgen "${parts[@]:0:${#parts[@]}-1}" -- "$current" 2>/dev/null
      )
    fi
  fi
  if ((${#COMPREPLY[@]} == 0)); then
    while IFS= read -r candidate; do COMPREPLY+=("$candidate"); done < <(
      if ((COMP_CWORD == 0)); then compgen -c -- "$current"; else compgen -f -- "$current"; fi
    )
  fi
  for candidate in "${COMPREPLY[@]}"; do
    ((${#_vlxc_values[@]} >= 64)) && break
    [[ -n $candidate ]] || continue
    path=$candidate tilde=''
    if [[ ${line:start:1} == '~' && $candidate == '~'* ]]; then
      tilde=${candidate%%/*}
      # Evaluate only a validated tilde prefix; the candidate's path is never evaluated.
      if [[ $tilde =~ ^~[a-zA-Z0-9_.+-]*$ ]]; then
        eval "path=$tilde"
        if [[ $path != "$tilde" ]]; then
          path+=${candidate:${#tilde}}
        else tilde='' path=$candidate
        fi
      else tilde=''
      fi
    fi
    [[ -d $path && $candidate != */ ]] && candidate+=/
    if [[ -n $tilde ]]; then
      quoted=$tilde
      # Keep the first slash unquoted so Bash still expands the tilde, even with $'...' quoting.
      if [[ $candidate == */* ]]; then
        quoted+=/
        if [[ -n ${candidate#*/} ]]; then
          LC_ALL=$quoting_locale printf -v suffix %q "${candidate#*/}"
          quoted+=$suffix
        fi
      fi
    else LC_ALL=$quoting_locale printf -v quoted %q "$candidate"
    fi
    _vlxc_values+=("${line:0:start}${quoted}${line:end}")
    _vlxc_cursors+=($((start + ${#quoted})))
    _vlxc_hex "$candidate"
    _vlxc_emit "C;$REPLY;"
  done
  _vlxc_prefix "$line" "$point"
  _vlxc_emit Z
  return "$saved_status"
}
# Like native Tab, offer the text all candidates share when it extends what was typed. Offsets are bytes,
# matching the candidate cursors computed above.
_vlxc_prefix() {
  local LC_ALL=C line=$1 point=$2 common head tail i byte need continuation=0
  _vlxc_prefix_value='' _vlxc_prefix_cursor=0
  ((${#_vlxc_values[@]} > 1)) || return 0
  common=${_vlxc_values[0]:0:${_vlxc_cursors[0]}}
  for ((i=1; i<${#_vlxc_values[@]}; i++)); do
    head=${_vlxc_values[i]:0:${_vlxc_cursors[i]}}
    while [[ -n $common && ${head:0:${#common}} != "$common" ]]; do common=${common:0:${#common}-1}; done
  done
  # Never stop inside a multibyte character, an escape sequence or ANSI-C quoting.
  for ((i=${#common}; i>0; i--)); do
    printf -v byte '%d' "'${common:i-1:1}"
    ((byte < 0)) && byte=$((byte + 256))
    if ((byte >= 128 && byte < 192)); then continuation=$((continuation + 1)); continue; fi
    if ((byte >= 192)); then
      need=$(( byte >= 240 ? 3 : byte >= 224 ? 2 : 1 ))
      ((continuation < need)) && common=${common:0:i-1}
    fi
    break
  done
  tail=${common##*[!\\]}
  ((${#tail} % 2)) && common=${common:0:${#common}-1}
  [[ ${common:point} == *"\$'"* ]] && return 0
  ((${#common} > point)) && [[ ${common:0:point} == "${line:0:point}" ]] || return 0
  _vlxc_prefix_value=$common${_vlxc_values[0]:${_vlxc_cursors[0]}}
  _vlxc_prefix_cursor=${#common}
  _vlxc_emit E
}
_vlxc_accept() {
  local revision index
  read -r revision index < "$_vlxc_selection" || return
  [[ $revision == "$_vlxc_revision" && ( $index =~ ^[0-9]+$ || $index == p ) ]] || return
  [[ $READLINE_LINE == "$_vlxc_buffer" && $READLINE_POINT == "$_vlxc_cursor" ]] || return
  if [[ $index == p ]]; then
    [[ -n $_vlxc_prefix_value ]] || return
    READLINE_LINE=$_vlxc_prefix_value READLINE_POINT=$_vlxc_prefix_cursor
    _vlxc_values=() _vlxc_prefix_value=''
    return
  fi
  ((index < ${#_vlxc_values[@]})) || return
  READLINE_LINE=${_vlxc_values[index]} READLINE_POINT=${_vlxc_cursors[index]}
  _vlxc_values=()
}
bind -x '"\e[24;5~":_vlxc_query'
bind -x '"\e[23;5~":_vlxc_accept'
# Keep the existing prompt command and its exit status intact.
if declare -p PROMPT_COMMAND 2>/dev/null | command grep -q 'declare -a'; then
  PROMPT_COMMAND+=('_vlxc_ready')
else
  PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND; }_vlxc_ready"
fi
if (( BASH_VERSINFO[0] > 4 || BASH_VERSINFO[1] >= 4 )); then
  # Literal protocol bytes work with promptvars disabled and preserve the user's PS0 expansion.
  printf -v PS0 '%s\e]6973;%s;X\a' "${PS0-}" "$_vlxc_nonce"
else
  # Older Bash has no pre-execution prompt hook.
  bind -x '"\e[99;1~":_vlxc_busy'
  bind '"\e[99;2~":accept-line'
  bind '"\C-m":"\e[99;1~\e[99;2~"'
  bind '"\C-j":"\e[99;1~\e[99;2~"'
fi
