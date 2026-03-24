#!/usr/bin/env bash
set -eu

calc() {
	command calc -qd "$@" | tr -d '[:blank:]'
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
		mapid=$(cut -d, -f1 <<<"$line")
		zfar=$(cut -d, -f2 <<<"$line")
		zfari=$(calc "(round($zfar / $prec) * $prec)")
		zfar=$(calc -q -d "($zfari / $factor)")
		#jq -Mc --argjson mapid "$mapid" '{"\($mapid)": { z: { $far, $far3072, } }}'
		printf '%s\n"%s":{"z":{"far":%d,"far3072":%s}}' "$first" "$mapid" "$zfari" "$zfar"
		first=,
	done < $projcsv
	printf '\n}\n'
}

CMD=convjson
if [[ $# -gt 0 ]]; then
	CMD=$1
	shift
fi
"do_${CMD}" "$@"
