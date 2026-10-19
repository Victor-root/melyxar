#!/usr/bin/env bash
#
# Builds ffmpeg-melyxar: libplacebo at the release named in `release`, then
# the FFmpeg release named there, checked against its SHA-256, with every
# patch of `patches/` applied in order and the options of `configure-options`.
#
#   build.sh WORK_DIR
#
# Leaves `ffmpeg` and `ffprobe` in WORK_DIR/bin and libplacebo in
# WORK_DIR/lib, which the tools find beside them wherever the two folders
# are put together. Putting them in place is the caller's business. Needs
# the packages listed in `packages`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$1"

# shellcheck source=release
source "${here}/release"

# The lines of a list file that are neither empty nor comments.
lines_of() {
  grep -v -e '^[[:space:]]*#' -e '^[[:space:]]*$' "$1"
}

deps="${work}/deps"
placebo="${work}/libplacebo"
git clone --quiet --depth 1 --branch "$LIBPLACEBO_TAG" \
  --recurse-submodules --shallow-submodules \
  https://code.videolan.org/videolan/libplacebo.git "$placebo"
found="$(git -C "$placebo" rev-parse HEAD)"
if [[ "$found" != "$LIBPLACEBO_COMMIT" ]]; then
  echo "libplacebo ${LIBPLACEBO_TAG} is ${found}, expected ${LIBPLACEBO_COMMIT}" >&2
  exit 1
fi
# Only what FFmpeg's filter uses: Vulkan, with shaderc to compile its shaders.
meson setup "${placebo}/build" "$placebo" --prefix="$deps" --libdir=lib \
  --buildtype=release -Ddefault_library=shared \
  -Dvulkan=enabled -Dshaderc=enabled -Dglslang=disabled \
  -Dopengl=disabled -Dd3d11=disabled -Ddemos=false -Dtests=false
ninja -C "${placebo}/build" install

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
  # libplacebo found where it was just built.
  PKG_CONFIG_PATH="${deps}/lib/pkgconfig${PKG_CONFIG_PATH:+:${PKG_CONFIG_PATH}}" \
    ./configure --extra-version="melyxar${MELYXAR_REVISION}" "${options[@]}"
  make -j"$(nproc)" ffmpeg ffprobe
)

mkdir -p "${work}/bin" "${work}/lib"
install -m 0755 "${src}/ffmpeg" "${src}/ffprobe" "${work}/bin/"
cp -P "${deps}"/lib/libplacebo.so* "${work}/lib/"
# Looked for at run time in the lib folder beside the tools' own, wherever
# the two are put. Set afterwards: a $ORIGIN does not survive the quoting
# of FFmpeg's configure and make.
patchelf --set-rpath '$ORIGIN/../lib' "${work}/bin/ffmpeg" "${work}/bin/ffprobe"
