#!/bin/bash
# Check that the tree is fit for the public repository. Prints every finding as `file:line: category: text` and exits
# 1 when there is any (0 when the tree is clean, 2 on a usage error).
#
#   scripts/check-public.sh [--summary] [PATH ...]
#
#   PATH        files or folders to check (default: the whole tree)
#   --summary   after the findings, print their count per category and per folder
#
# Files: the tracked and the untracked, not ignored files of the git checkout (outside git: every file under the
# repository root except target/ and .git/). Binary files are skipped by the text checks. This script itself is not
# checked (it holds the patterns).
#
# Categories:
#   local-path   folders of a development machine: an E: drive, Windows / macOS user folders other than the `<you>`
#                placeholder, home folders other than the examples /home/me, /home/you, /home/user; plus the regex in
#                CHECK_PUBLIC_EXTRA when it is set (your own user name, the folder name of a private checkout, ...)
#   private-ref  private notes: research / session / scratch files, the plan file, run captures (`runs/r12`,
#                "r53 logs", "(r61)")
#   decision     "owner" as the source of a rule or a decision ("owner decision", "owner-approved", "owner 2026-..",
#                "the owner wants", "(owner)"); "owner" meaning the owner of an item, a decal or a lock is fine
#   disassembly  instructions and registers (`mov`, `lea`, `rax`, `[rsp+0x58]`, `r12d`): keep the address and say what
#                the value means instead
#   email        e-mail addresses other than example.com / example.org / example.net
#   ip           IPv4 addresses other than 127.x, 0.0.0.0, 255.255.255.255 and the documentation ranges 192.0.2.x,
#                198.51.100.x, 203.0.113.x (home-network addresses 10.x / 172.16-31.x / 192.168.x only in *.md guides)
#   private-key  a PEM private key block, unless the file is a test (in a tests/ folder or named *test*) or a line
#                among the 5 before the block says it is a test key
#   game-data    committed game data: data/*.json|tsv, prewarm/*.tsv, *.db, *.exe, *.dll, resources.*
set -u
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SELF=scripts/check-public.sh
SUMMARY=0
paths=()
while [ $# -gt 0 ]; do
  case "$1" in
    --summary) SUMMARY=1; shift ;;
    -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
    -*) echo "unknown option $1 (see --help)"; exit 2 ;;
    *) paths+=("$1"); shift ;;
  esac
done
cd "$ROOT" || exit 2

FILES=()
if git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
  while IFS= read -r -d '' f; do [ -f "$f" ] && FILES+=("$f"); done < <(git ls-files -co --exclude-standard -z -- "${paths[@]}")
else
  while IFS= read -r -d '' f; do FILES+=("${f#./}"); done < <(find "${paths[@]:-.}" -type f -not -path '*/target/*' -not -path '*/.git/*' -print0)
fi
TEXT=()
for f in "${FILES[@]}"; do [ "$f" = "$SELF" ] || TEXT+=("$f"); done

OUT=$(mktemp)
trap 'rm -f "$OUT"' EXIT

# scan CATEGORY GREP_FLAGS REGEX: every matching line as `file:line: CATEGORY: text`
scan() {
  [ ${#TEXT[@]} = 0 ] && return
  printf '%s\0' "${TEXT[@]}" | xargs -0 grep -n -H -I $2 -E -e "$3" -- 2> /dev/null | tr -d '\r' |
    sed -E "s/^([^:]*:[0-9]+):[[:space:]]*/\1: $1: /" | cut -c1-240
}

# 1. local paths
scan local-path "" '/mnt/e/|\bE:\\|\bE:/|C:\\+Users\\+[^<\\]|/Users/[A-Za-z0-9_.-]+/' >> "$OUT"
scan local-path "" '/home/[A-Za-z0-9_.-]+' | while IFS= read -r l; do
  rest=$(printf '%s' "${l#*: local-path: }" | grep -o -E '/home/[A-Za-z0-9_.-]+' | grep -v -x -E '/home/(me|you|user)')
  [ -n "$rest" ] && printf '%s\n' "$l"
done >> "$OUT"
[ -n "${CHECK_PUBLIC_EXTRA:-}" ] && scan local-path "" "$CHECK_PUBLIC_EXTRA" >> "$OUT"

# 2. private research / session / plan / run references
scan private-ref "" 'private/|v0[0-9]{2}[a-z]-research|research-[A-Za-z0-9_.-]+\.md|session-20[0-9]{2}-|\bscratch/|PLAN-0[0-9]{2}|PLAN\.md|GAME-SESSION|\bruns?/r[0-9]+|\b(runs?|logs?|captures?|traces?) r[0-9]{1,3}\b|\br[0-9]{1,3}( (logs?|trace|capture|runs?)\b|\.\.r?[0-9]{1,3}\b| ?/ ?r[0-9]{1,3}\b)|\(r[0-9]{2,3}[:),]|\(r[0-9]{2,3} ' >> "$OUT"

# 3. "owner" as the source of a decision (not the owner of an item)
scan decision "-i" "\bowners?[- ](decisions?|choices?|rules?|calls?|approved|approval|runs?|tests?|bugs?|checks?|cases?|seen|said|says|wants?|wanted|asked|asks|chose|decided|confirmed|agreed|picked|prefers|requests?|sessions?|machine|pc|answer)\b|\bowner-$|\bowner's$|\bowner +([A-Z][0-9]|[0-9]{4})|\bowner, 20[0-9]{2}|\bowner: (\"|yes\b|no\b|[a-z]+ [a-z]+)|(^| )\(owner(\)|:|;|, [a-z]+ [a-zA-Z0-9]+|$)|[ (]owner$|\bowner's (\"|decision|choice|rule|call|request|client|game|run|test|pvp|pc|machine|answer|case|live|kit|recording|finished|failed)|\b(for|to|ask|tell|asked|told|by) the owner\b|\bthe owner (said|says|wants|wanted|asked|asks|chose|decided|approved|confirmed|agreed|prefers|picked|should|can|could|will|may|must|runs|ran|played|saw|sees|did|set|reopened|tested)\b" >> "$OUT"

# 4. disassembly: registers, memory operands, instructions with operands
# (in Rust files only in comments: asm! blocks and emitted machine code are code)
# OWN_ASM: comments about OUR OWN machine code, not the game's, may name instructions and registers. Each entry is
# `file|regex|reason`: a disassembly finding in that file is dropped only when its text (leading blanks removed)
# matches the regex. Keep entries narrow; add one only for code we generate or registers our own tools take.
OWN_ASM=(
  'servers/world-host/src/thunks.rs|^//!  |the module doc lists the import thunk the host generates'
  'servers/world-host/src/thunks.rs|^c\.(push|extend_from_slice)\(|the thunk bytes the host emits, each named by its instruction'
  'servers/world-host/src/bp.rs|^/// watchpoint on reg\+off \(reg: |the register names the HOST_ARMW debug flag accepts'
)
own_asm() {  # FILE TEXT: 0 when an OWN_ASM entry allows the finding
  local e f re
  for e in "${OWN_ASM[@]}"; do
    f=${e%%|*}; re=${e#*|}; re=${re%|*}
    [ "$1" = "$f" ] && printf '%s' "$2" | grep -q -E -e "$re" && return 0
  done
  return 1
}
scan disassembly "" '\b(rax|rbx|rcx|rdx|rsi|rdi|rsp|rbp|rip|eax|ebx|ecx|edx|esi|edi|esp|ebp|xmm[0-9]{1,2})\b|\br(8|9|1[0-5])[dwb]\b|\b(mov|movzx|movsx|movsxd|movss|movsd|movaps|movups|lea|cmp|test|xor|and|or|sub|add|imul|shl|shr|sar|call|push|pop|jmp|inc|dec|cmovn?[a-z]{1,2}|set[a-z]{1,3})\s+(byte |word |dword |qword )?(ptr )?\[[A-Za-z0-9_]+[+-]' | while IFS= read -r l; do
  case "${l%%:*}" in
    *.rs) c=${l#*: disassembly: }; own_asm "${l%%:*}" "$c" && continue; case "$c" in *//*) printf '%s' "${c#*//}" | grep -q -E -e '\b(rax|rbx|rcx|rdx|rsi|rdi|rsp|rbp|rip|eax|ebx|ecx|edx|esi|edi|esp|ebp|xmm[0-9]{1,2})\b|\br(8|9|1[0-5])[dwb]\b|\b(mov|movzx|movsx|movsxd|movss|movsd|movaps|movups|lea|cmp|test|xor|and|or|sub|add|imul|shl|shr|sar|call|push|pop|jmp|inc|dec|cmovn?[a-z]{1,2}|set[a-z]{1,3})\s+(byte |word |dword |qword )?(ptr )?\[[A-Za-z0-9_]+[+-]' && printf '%s\n' "$l" ;; esac ;;
    *) printf '%s\n' "$l" ;;
  esac
done >> "$OUT"

# 5. e-mail addresses (example domains are fine)
scan email "" '[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}' |
  grep -v -i -E '@example\.(com|org|net)\b' | while IFS= read -r l; do
    rest=$(printf '%s' "$l" | grep -o -E '[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}' | grep -v -i -E '@example\.(com|org|net)$')
    [ -n "$rest" ] && printf '%s\n' "$l"
  done >> "$OUT"

# 6. IPv4 addresses outside localhost and the documentation ranges
ip_ok() {
  case "$1" in
    127.*|0.0.0.0|255.255.255.255|192.0.2.*|198.51.100.*|203.0.113.*) return 0 ;;
    10.*|192.168.*|172.1[6-9].*|172.2[0-9].*|172.3[01].*) case "$2" in *.md) return 0 ;; esac ;;
  esac
  return 1
}
scan ip "" '(^|[^0-9.])([0-9]{1,3}\.){3}[0-9]{1,3}($|[^0-9.-])' | while IFS= read -r l; do
  file=${l%%:*}
  text=${l#*: ip: }
  bad=0
  for a in $(printf '%s' "$text" | grep -o -E '(^|[^0-9.])([0-9]{1,3}\.){3}[0-9]{1,3}($|[^0-9.-])' | grep -o -E '([0-9]{1,3}\.){3}[0-9]{1,3}'); do
    ip_ok "$a" "$file" || bad=1
  done
  [ "$bad" = 1 ] && printf '%s\n' "$l"
done >> "$OUT"

# 7. private keys outside marked test files
for f in "${TEXT[@]}"; do
  grep -q -I -E -e '-----BEGIN [A-Z ]*PRIVATE KEY-----' -- "$f" 2> /dev/null || continue
  case "$f" in */tests/*|tests/*|*test*) continue ;; esac
  tr -d '\r' < "$f" | awk -v f="$f" '
    { prev[NR % 6] = $0 }
    /-----BEGIN [A-Z ]*PRIVATE KEY-----/ {
      marked = 0
      for (i = 1; i <= 5; i++) if (tolower(prev[(NR - i) % 6]) ~ /test/) marked = 1
      if (!marked) printf "%s:%d: private-key: %s\n", f, NR, substr($0, 1, 160)
    }' >> "$OUT"
done

# 8. committed game data (file names; binary files too)
for f in "${FILES[@]}"; do
  case "$f" in
    data/*.json|data/*.tsv|*/data/*.json|*/data/*.tsv|prewarm/*.tsv|*/prewarm/*.tsv|*.db|*.exe|*.dll|*.EXE|*.DLL|resources.*|*/resources.*)
      echo "$f:0: game-data: committed game data file" >> "$OUT" ;;
  esac
done

n=$(wc -l < "$OUT")
sort -t: -k1,1 -k2,2n "$OUT"
if [ "$SUMMARY" = 1 ] && [ "$n" != 0 ]; then
  echo
  echo "per category:"
  sed -E 's/^[^:]*:[0-9]+: ([a-z-]+): .*/\1/' "$OUT" | sort | uniq -c | sort -rn
  echo "per folder:"
  sed -E 's#^((servers|crates)/[^/:]+|[^/:]+)[/:].*#\1#' "$OUT" | sort | uniq -c | sort -rn
fi
if [ "$n" != 0 ]; then
  echo "check-public: $n finding(s)"
  exit 1
fi
echo "check-public: clean (${#FILES[@]} files)"
