#!/bin/bash
# TEST STUB of /home/admin/_cache.sh (RaspiBlitz keeps these values in redis)
store=/var/cache/stub
case "$1" in
  get)
    shift
    echo "# _cache.sh get $*"
    for key in "$@"; do
      echo "${key}=\"$(cat "${store}/${key}" 2>/dev/null)\""
    done
    ;;
  set)
    printf '%s' "$3" > "${store}/$2"
    ;;
  *)
    exit 0
    ;;
esac
