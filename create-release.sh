#!/usr/bin/env bash

set -e

export CARGO_REGISTRY_TOKEN=$(cat ./crates-io.token)

PACKAGE=hoolamike
VERSION=$(cargo pkgid --package $PACKAGE | cut -d "#" -f2)
TAG="v${VERSION}"

echo "creating release for ${PACKAGE} ${TAG}"

# cargo publish --package "${PACKAGE}"
git cliff -o CHANGELOG.md
git add .
git commit -m "${PACKAGE} release ${TAG}"
git tag -a "${TAG}" -m "release ${TAG} of ${PACKAGE}"

git push origin $(git rev-parse --abbrev-ref HEAD) --tags
