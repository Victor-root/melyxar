#!/usr/bin/env bash
#
# Builds ffmpeg-melyxar: the FFmpeg release named in `release`, checked
# against its SHA-256, with every patch of `patches/` applied in order and
# the options of `configure-options`.
#
#   build.sh WORK_DIR
#
# Leaves `ffmpeg` and `ffprobe` in WORK_DIR/bin. Putting them in place is the
# caller's business. Needs the packages listed in `packages`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$1"

# shellcheck source=release
source "${here}/release"

# The lines of a list file that are neither empty nor comments.
lines_of() {
  grep -v -e '^[[:space:]]*#' -e '^[[:space:]]*$' "$1"
}

archive="ffmpeg-${FFMPEG_VERSION}.tar.xz"
mkdir -p "$work"
curl -fsSL --retry 3 -o "${work}/${archive}" "https://ffmpeg.org/releases/${archive}"
echo "${FFMPEG_SHA256}  ${work}/${archive}" | sha256sum --check --quiet

tar -xf "${work}/${archive}" -C "$work"
src="${work}/ffmpeg-${FFMPEG_VERSION}"

for change in "${here}"/patches/*.patch; do
  [[ -e "$change" ]] || continue
  patch -d "$src" -p1 --forward --quiet < "$change"
done

mapfile -t options < <(lines_of "${here}/configure-options")
(
  cd "$src"
  ./configure --extra-version="melyxar${MELYXAR_REVISION}" "${options[@]}"
  make -j"$(nproc)" ffmpeg ffprobe
)

mkdir -p "${work}/bin"
install -m 0755 "${src}/ffmpeg" "${src}/ffprobe" "${work}/bin/"
