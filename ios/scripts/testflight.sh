#!/bin/bash
# Upload the iPhone app to TestFlight and hand it to the external group "Team"
# (the one with the public link): set "What to Test", add the build to the
# group and submit it for beta review.
#
#   ios/scripts/testflight.sh             # archive, upload, then distribute
#   ios/scripts/testflight.sh distribute  # only distribute the newest uploaded build of this version
#   ios/scripts/testflight.sh check       # see that the API key works
#
# WHAT_TO_TEST="..." replaces the generated notes. The API key settings live
# outside the repo in ~/.appstoreconnect/workboard.env. See docs/ios-design.md,
# "TestFlight".
set -euo pipefail
cd "$(dirname "$0")/../.."

env_file="$HOME/.appstoreconnect/workboard.env"
if [ -f "$env_file" ]; then
	set -a
	# shellcheck disable=SC1090
	. "$env_file"
	set +a
fi

asc() { swift ios/scripts/asc.swift "$@"; }

version=$(sed -n 's/.*MARKETING_VERSION = \([^;]*\);.*/\1/p' ios/Workboard.xcodeproj/project.pbxproj | head -1)
out="${WB_IOS_OUT:-/tmp/wb-ios}"
group="${WB_TESTFLIGHT_GROUP:-Team}"

# iPhone changes since the last bump to a different iPhone version.
write_notes() {
	if [ -n "${WHAT_TO_TEST:-}" ]; then
		printf '%s\n' "$WHAT_TO_TEST" >"$1"
		return
	fi
	local prev changes
	prev=$(git log --format='%H %s' --grep='^Bump the iPhone app to' |
		awk -v v="$version" '$NF != v { print $1; exit }')
	changes=$(git log --no-merges --format='- %s' ${prev:+"$prev"..}HEAD -- ios/ |
		grep -v '^- Bump the iPhone app' | head -30 || true)
	{
		echo "Workboard $version"
		echo "${changes:-- Bug fixes and small improvements}"
	} >"$1"
}

distribute() {
	mkdir -p "$out"
	write_notes "$out/what-to-test.txt"
	echo "What to Test:"
	sed 's/^/  /' "$out/what-to-test.txt"
	asc distribute --version "$version" --group "$group" \
		--notes-file "$out/what-to-test.txt" "$@"
}

case "${1:-upload}" in
check)
	asc check
	;;
distribute)
	distribute
	;;
upload)
	# Fail on a missing or wrong key before the long build.
	asc check >/dev/null
	since=$(date -u +%Y-%m-%dT%H:%M:%SZ)
	rm -rf "$out/Workboard.xcarchive" "$out/export"
	xcodebuild -project ios/Workboard.xcodeproj -scheme Workboard -configuration Release \
		-destination 'generic/platform=iOS' -archivePath "$out/Workboard.xcarchive" \
		-allowProvisioningUpdates archive
	xcodebuild -exportArchive -archivePath "$out/Workboard.xcarchive" \
		-exportOptionsPlist ios/Config/ExportOptions.plist -exportPath "$out/export" \
		-allowProvisioningUpdates
	echo "Uploaded $version. Waiting for App Store Connect to process it."
	distribute --since "$since"
	;;
*)
	echo "usage: $0 [upload|distribute|check]" >&2
	exit 1
	;;
esac
