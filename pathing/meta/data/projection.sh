#!/usr/bin/env bash
set -eu

calc() {
	command calc -D:0 -qd "$@" | tr -d '[:blank:]'
}

ZFACTOR=3072
do_convjson() {
	local projcsv=projection.csv line mapid \
		zfar zfari prec=12 factor=$ZFACTOR first=
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
	done < <(sort -ugt, <(settingsproj "${ZSETTINGS-}") $projcsv)
	printf '\n}\n'
}

settingsproj() {
	local settings
	settings=$1
	if [[ -z $settings ]]; then return; fi
	jq -er --argjson zfactor "$ZFACTOR" '.pathing.space.goggles0.map_proj_seen | . as $proj | keys | .[] | . as $key | "\($key),\($proj[$key].z.farz * $zfactor)"' "$settings"
}

CMD=convjson
if [[ $# -gt 0 ]]; then
	CMD=$1
	shift
fi
"do_${CMD}" "$@"
