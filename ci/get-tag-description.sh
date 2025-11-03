#!/usr/bin/env bash
set -euxo pipefail

tag_name_for() {
	if [[ $1 = refs/tags/* ]]; then
		printf '%s\n' "${1#refs/tags/}"
	else
		git describe --tags --abbrev=0 "$1"
	fi
}
TAG_REF=${TAG_REF:-${GITHUB_REF:-HEAD}}
TAG=$(tag_name_for "$TAG_REF")

RELEASE_REF=${RELEASE_REF:-$TAG_REF}
RELEASE_TAG=$(tag_name_for "$RELEASE_REF")

echo 'TAG_SUBJECT<<EOF' >> $GITHUB_OUTPUT
REFNAME_RELEASE=
if [[ $RELEASE_TAG != $TAG ]]; then
	REFNAME_RELEASE=" ($RELEASE_TAG)"
fi
git tag -n1 "$TAG" --format='Release! %(refname:lstrip=2)'"${REFNAME_RELEASE} - %(contents:subject)" >> $GITHUB_OUTPUT
echo 'EOF' >> $GITHUB_OUTPUT
echo 'TAG_BODY<<EOF' >> $GITHUB_OUTPUT
git tag -n1 "$TAG" --format='%(contents:body)' >> $GITHUB_OUTPUT
echo 'EOF' >> $GITHUB_OUTPUT
echo "TAG_URL=https://github.com/TaimiHUD/TaimiHUD/releases/tag/${RELEASE_TAG}" >> $GITHUB_OUTPUT
