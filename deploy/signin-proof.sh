#!/bin/sh
# signin-proof.sh URL: signs a kid in through the public tunnel, end to end, on a throwaway class.
# Waits for URL/v1/health, makes a class with the teacher key, reads its roster by class code, makes the first
# picture pick, signs in with the right picture, reads the kid's record, checks a wrong picture is refused, checks
# URL/nature-words/ is the lesson page, then deletes the class. Any miss exits non-zero; the class goes either way.
set -eu
url=${1:?usage: signin-proof.sh URL}
url=${url%/}
key=$(cat "$HOME/.config/lesson-server/teacher.key")
# Each request gets 10 s (curl -m 10): through a live quick tunnel one answered in 0.03 to 0.78 s (5 tries, measured).
fail() { echo "proof: FAIL: $*" >&2; exit 1; }

# Measured: a fresh quick tunnel answered /v1/health after 9.4, 17.0 and 40.6 s (3 restarts); 60 s is the worst
# of those and half again.
deadline=$(( $(date +%s) + 60 ))
until curl -fsS -m 10 "$url/v1/health" 2>/dev/null | grep -q healthy; do
  [ "$(date +%s)" -lt "$deadline" ] || fail "$url/v1/health did not answer within 60 s"
  sleep 1
done
echo "proof: $url answers"

code=$(curl -fsS -m 10 -H "X-Teacher-Key: $key" -H 'Content-Type: application/json' \
  -d '{"name":"proof class","students":["Noa B","Omer"]}' "$url/api/classes" | jq -r .code)
[ -n "$code" ] && [ "$code" != null ] || fail "could not make a class"
cleanup() { curl -fsS -m 10 -X DELETE -H "X-Teacher-Key: $key" "$url/api/classes/$code" >/dev/null || echo "proof: could not delete class $code" >&2; }
trap cleanup EXIT

kid=$(curl -fsS -m 10 "$url/api/roster/$code" | jq -r '.students[] | select(.name=="Noa B") | .id')
[ -n "$kid" ] || fail "roster by class code has no Noa B"
echo "proof: roster read by class code"

pick=$(curl -fsS -m 10 -H 'Content-Type: application/json' -d "{\"code\":\"$code\",\"student\":$kid,\"picture\":3}" "$url/api/pick" | jq -r .token)
[ -n "$pick" ] && [ "$pick" != null ] || fail "first picture pick gave no token"
echo "proof: first picture picked"

token=$(curl -fsS -m 10 -H 'Content-Type: application/json' -d "{\"code\":\"$code\",\"student\":$kid,\"picture\":3}" "$url/api/signin" | jq -r .token)
[ -n "$token" ] && [ "$token" != null ] || fail "right picture gave no token"
echo "proof: signed in with the right picture"

curl -fsS -m 10 -H "Authorization: Bearer $token" "$url/api/me" | grep -q 'Noa B' || fail "own record not readable with the token"
echo "proof: own record read"

wrong=$(curl -sS -m 10 -o /dev/null -w '%{http_code}' -H 'Content-Type: application/json' \
  -d "{\"code\":\"$code\",\"student\":$kid,\"picture\":5}" "$url/api/signin")
[ "$wrong" = 401 ] || fail "wrong picture answered $wrong, not 401"
echo "proof: wrong picture refused (401)"

page=$(curl -fsS -m 10 "$url/nature-words/") || fail "$url/nature-words/ did not answer"
printf '%s' "$page" | grep -q '<title>Nature Trails' || fail "$url/nature-words/ is not the lesson page"
echo "proof: lesson page served"
echo "proof: OK"
