#!/bin/sh
# signin-proof.sh URL: a new kid joins and signs in through the public tunnel, end to end, on a throwaway empty class.
# Waits for URL/v1/health, makes an empty class with the teacher key and opens joining, joins "Noa B." by the API,
# checks the roster hides her and a pick is refused until the teacher approves her, then that her ticket says
# approved, picks her animal, signs in, reads her record, checks a wrong animal is refused, posts a race score and
# reads her approved name on the board, checks a free name on the open board is refused, a rejected name's ticket is
# gone, a two-word name is refused, a join after joining closes is refused, and URL/nature-words/ is the lesson page.
# Any miss exits non-zero; the class is deleted either way.
set -eu
url=${1:?usage: signin-proof.sh URL}
url=${url%/}
key=$(cat "$HOME/.config/lesson-server/teacher.key")
# Each request gets 10 s (curl -m 10): through a live quick tunnel one answered in 0.03 to 0.78 s (5 tries, measured).
fail() { echo "proof: FAIL: $*" >&2; exit 1; }
# status METHOD PATH [JSON] [HEADER]: the HTTP status only.
status() {
  curl -sS -m 10 -o /dev/null -w '%{http_code}' -X "$1" -H 'Content-Type: application/json' ${4:+-H "$4"} ${3:+-d "$3"} "$url$2"
}
teach() { curl -fsS -m 10 -X "$1" -H "X-Teacher-Key: $key" -H 'Content-Type: application/json' ${3:+-d "$3"} "$url$2"; }

# Measured: a fresh quick tunnel answered /v1/health after 9.4, 17.0 and 40.6 s (3 restarts); 60 s is the worst
# of those and half again.
deadline=$(( $(date +%s) + 60 ))
until curl -fsS -m 10 "$url/v1/health" 2>/dev/null | grep -q healthy; do
  [ "$(date +%s)" -lt "$deadline" ] || fail "$url/v1/health did not answer within 60 s"
  sleep 1
done
echo "proof: $url answers"

code=$(teach POST /api/classes '{"name":"proof class","students":[]}' | jq -r .code)
[ -n "$code" ] && [ "$code" != null ] || fail "could not make an empty class"
cleanup() { curl -fsS -m 10 -X DELETE -H "X-Teacher-Key: $key" "$url/api/classes/$code" >/dev/null || echo "proof: could not delete class $code" >&2; }
trap cleanup EXIT

teach POST "/api/classes/$code/join" '{"open":true}' | grep -q join_s || fail "could not open joining"
echo "proof: empty class made, joining open"

ticket=$(curl -fsS -m 10 -H 'Content-Type: application/json' -d "{\"code\":\"$code\",\"name\":\"Noa B.\"}" "$url/api/join" | jq -r .ticket)
[ -n "$ticket" ] && [ "$ticket" != null ] || fail "joining as Noa B. gave no ticket"
curl -fsS -m 10 "$url/api/join/$ticket" | jq -e '.status=="pending"' >/dev/null || fail "the ticket does not say pending"
kid=$(teach GET "/api/classes/$code/students" | jq -r '.students[] | select(.name=="Noa B." and .status=="pending") | .id')
[ -n "$kid" ] || fail "the teacher's list has no waiting Noa B."
curl -fsS -m 10 "$url/api/roster/$code" | jq -e '.students | length == 0' >/dev/null || fail "the roster lists a name still waiting"
s=$(status POST /api/pick "{\"code\":\"$code\",\"student\":$kid,\"picture\":3}")
[ "$s" = 404 ] || fail "a waiting kid's pick answered $s, not 404"
echo "proof: Noa B. waits; the roster hides her and her pick is refused"

teach POST "/api/students/$kid/approve" >/dev/null || fail "approve failed"
curl -fsS -m 10 "$url/api/join/$ticket" | jq -e ".status==\"approved\" and .id==$kid" >/dev/null || fail "the ticket does not say approved"
curl -fsS -m 10 "$url/api/roster/$code" | jq -e '.students[0].name=="Noa B."' >/dev/null || fail "the roster does not list Noa B. after approval"
echo "proof: the teacher approved Noa B."

pick=$(curl -fsS -m 10 -H 'Content-Type: application/json' -d "{\"code\":\"$code\",\"student\":$kid,\"picture\":3}" "$url/api/pick" | jq -r .token)
[ -n "$pick" ] && [ "$pick" != null ] || fail "first picture pick gave no token"
token=$(curl -fsS -m 10 -H 'Content-Type: application/json' -d "{\"code\":\"$code\",\"student\":$kid,\"picture\":3}" "$url/api/signin" | jq -r .token)
[ -n "$token" ] && [ "$token" != null ] || fail "right picture gave no token"
curl -fsS -m 10 -H "Authorization: Bearer $token" "$url/api/me" | jq -e '.name=="Noa B."' >/dev/null || fail "own record does not say Noa B."
echo "proof: picked, signed in, own record says Noa B."

s=$(status POST /api/signin "{\"code\":\"$code\",\"student\":$kid,\"picture\":5}")
[ "$s" = 401 ] || fail "wrong picture answered $s, not 401"
echo "proof: wrong picture refused (401)"

# The race board topic is this proof's own, so the class's boards never see it.
topic="proof-$code"
curl -fsS -m 10 -H "Authorization: Bearer $token" -H 'Content-Type: application/json' \
  -d "{\"topic\":\"$topic\",\"place\":1,\"score\":7}" "$url/api/race" >/dev/null || fail "POST /api/race failed"
curl -fsS -m 10 "$url/$topic/json?poll=1&since=12h" | jq -e 'select(.message | fromjson | .n=="Noa B." and .s==7)' >/dev/null \
  || fail "the board does not show Noa B.'s score"
s=$(status POST "/$topic" '{"n":"Anyone","s":9,"p":1,"t":1}')
[ "$s" = 403 ] || fail "a free name on the open board answered $s, not 403"
echo "proof: the board shows the approved name; a free name is refused (403)"

t2=$(curl -fsS -m 10 -H 'Content-Type: application/json' -d "{\"code\":\"$code\",\"name\":\"Omer\"}" "$url/api/join" | jq -r .ticket)
k2=$(teach GET "/api/classes/$code/students" | jq -r '.students[] | select(.name=="Omer") | .id')
[ -n "$k2" ] || fail "Omer is not waiting"
teach POST "/api/students/$k2/reject" >/dev/null || fail "reject failed"
s=$(status GET "/api/join/$t2")
[ "$s" = 404 ] || fail "a rejected name's ticket answered $s, not 404"
echo "proof: a rejected name's ticket is gone (404)"

s=$(status POST /api/join "{\"code\":\"$code\",\"name\":\"Noa Ben\"}")
[ "$s" = 400 ] || fail "a two-word name answered $s, not 400"
teach POST "/api/classes/$code/join" '{"open":false}' >/dev/null || fail "could not close joining"
s=$(status POST /api/join "{\"code\":\"$code\",\"name\":\"Dana\"}")
[ "$s" = 403 ] || fail "a join after joining closed answered $s, not 403"
echo "proof: a two-word name refused (400); joining closed refuses a join (403)"

page=$(curl -fsS -m 10 "$url/nature-words/") || fail "$url/nature-words/ did not answer"
printf '%s' "$page" | grep -q '<title>Nature Trails' || fail "$url/nature-words/ is not the lesson page"
echo "proof: lesson page served"
echo "proof: OK"
