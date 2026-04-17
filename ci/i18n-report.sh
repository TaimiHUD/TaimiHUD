#!/usr/bin/env bash
set -eu

FL_ROOT=${FLAKE_ROOT-.}/i18n
FL_EN=$FL_ROOT/en

fl_langs() {
	(cd "$FL_ROOT"; echo *)
}
fl_keys() {
	local fl_lang
	fl_lang=$1; shift
	ftl_keys "$@" "$FL_ROOT/$fl_lang"/*.ftl
}
fl_value() {
	local fl_lang
	fl_lang=$1; shift
	ftl_value "$@" "$FL_ROOT/$fl_lang"/*.ftl
}

ftl_keys() {
	grep -o '^[-a-zA-Z0-9_]\+ *=' "$@" | cut -d' ' -f1
}
ftl_value() {
	local ftl_path ftl_key ftl_line ftl_lineno=0 ftl_value_maxlines=${FTL_VALUE_MAXLINES-20}
	ftl_key=$1; shift
	ftl_path=("$@")

	while read -r ftl_line; do
		if [[ $ftl_lineno -eq 0 ]]; then
			ftl_line=$(cut -d= -f2- <<<"$ftl_line")
		elif [[ $ftl_line = \#* ]]; then
			continue
		elif [[ $ftl_line =~ ^[-a-zA-Z0-9_]+\ *= ]]; then
			break
		fi
		while [[ $ftl_line = \ * ]]; do
			ftl_line=${ftl_line:1}
		done
		printf '%s\n' "$ftl_line"
		ftl_lineno=$((ftl_lineno+1))
	done < <(grep -A "$ftl_value_maxlines" "^$ftl_key *=" "${ftl_path[@]}" 2>/dev/null)
}

fl_report() {
	local fl_langs fl_lang fl_key fl_value fl_fallback_count \
		fl_keys all_keys en_keys report_fmt=${FL_REPORT_FMT-txt}
	if [[ $# -gt 0 ]]; then
		fl_langs=($@)
	else
		fl_langs=($(fl_langs))
	fi
	en_keys=($(fl_keys en))
	for fl_lang in "${fl_langs[@]}"; do
		if [[ $fl_lang = en ]]; then
			fl_keys=("${en_keys[@]}");
		else
			fl_keys=($(fl_keys "$fl_lang"))
		fi
		case $report_fmt in
			txt)
				printf ':: %s (%d keys)\n' "$fl_lang" "${#fl_keys[@]}"
				;;
			html)
				printf '<h2>%s</h2>\n' "$fl_lang"
				printf '<details><summary>%d keys</summary><ul>\n' "${#fl_keys[@]}"
				printf '<li><pre style="display:inline;">%s</pre></li>\n' "${fl_keys[@]}"
				printf '</ul></details>\n'
				;;
			*)
				echo "unrecognized report format \"$report_fmt\"" >&2
				exit 1
				;;
		esac
		if [[ $fl_lang = en ]]; then continue; fi

		case $report_fmt in
			html)
				printf '<h3>Progress</h3>\n'
				printf '<h4>Untranslated Fallbacks</h4>\n<ul>'
				;;
			*)
				printf "untranslated/fallbacks:\n"
				;;
		esac
		fl_fallback_count=0
		all_keys=$(printf ' %s ' "${fl_keys[@]}")
		for fl_key in "${en_keys[@]}"; do
			if [[ $all_keys = *" $fl_key "* ]]; then
				continue
			fi
			IFS=$'\n' fl_value=($(fl_value en "$fl_key"))
			case $report_fmt in
				html)
					printf '<li><details><summary><pre style="display:inline;">%s =</pre></summary>\n<blockquote>' "$fl_key"
					if [[ "${fl_value[*]}" != *'https://'* ]]; then
						printf '%s<br/>' "${fl_value[@]}"
					fi
					printf '</blockquote></details></li>\n'
					;;
				*)
					printf '* %s =\n' "$fl_key"
					printf '    %s\n' "${fl_value[@]}"
					;;
			esac
			fl_fallback_count=$((fl_fallback_count+1))
		done
		case $report_fmt in
			html)
				printf '</ul><p>(%d total)</p>\n' "$fl_fallback_count"
				;;
			*)
				printf "(%d total)\n" "$fl_fallback_count"
				;;
		esac
	done
}

fl_report "$@"
