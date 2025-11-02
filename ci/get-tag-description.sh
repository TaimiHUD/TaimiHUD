#!/usr/bin/env bash
set -euxo pipefail

git fetch --tags --force
BRANCH=${GITHUB_REF:-$(git branch --show-current)}
BRANCH=${BRANCH#refs/*/}
echo 'TAG_SUBJECT<<EOF' >> $GITHUB_OUTPUT
git tag -n1 "$BRANCH" --format=$'Release! %(refname:lstrip=2) - %(contents:subject)' >> $GITHUB_OUTPUT
echo 'EOF' >> $GITHUB_OUTPUT
echo 'TAG_BODY<<EOF' >> $GITHUB_OUTPUT
git tag -n1 "$BRANCH" --format=$'%(contents:body)' >> $GITHUB_OUTPUT
echo 'EOF' >> $GITHUB_OUTPUT
echo "TAG_URL=https://github.com/TaimiHUD/TaimiHUD/releases/tag/${BRANCH}" >> $GITHUB_OUTPUT
