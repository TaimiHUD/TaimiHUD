#!/usr/bin/env bash
set -eu

calc() {
	command calc -D:0 -qd "$@" | tr -d '[:blank:]'
}

do_convjson() {
	local projcsv=projection.csv line mapid \
		zfar zfari prec=12 factor=3072 first=
	if [[ $# -gt 0 ]]; then
		projcsv=$1
		shift
	fi
	printf '{'
	while read -r line; do
		mapid=$(cut -d, -f1 <<<"$line" | sed -e 's/^0\+//')
		zfar=$(cut -d, -f2 <<<"$line")
		zfari=$(calc "(round($zfar / $prec) * $prec)")
		zfar=$(calc -q -d "($zfari / $factor)")
		#jq -Mc --argjson mapid "$mapid" '{"\($mapid)": { z: { $far, $far3072, } }}'
		printf '%s\n"%d":{"z":{' "$first" "$mapid"
		first=,
		#printf '"far":%d,' "$zfari"
		printf '"farz":%s' "$zfar"
		printf '}}'
	done < $projcsv
	printf '\n}\n'
}

CMD=convjson
if [[ $# -gt 0 ]]; then
	CMD=$1
	shift
fi
"do_${CMD}" "$@"
