#!/usr/bin/env bash
#
# Builds Melyxar's test films: free films downloaded as they are (`downloads`),
# then, made from them, the cases no free film carries: picture and styled
# subtitles, interlaced pictures, uncommon frame rates, TrueHD and DTS sound,
# several sound tracks, chapters, forced and external subtitles, old and
# broken files, a very long one.
#
#   build.sh FFMPEG TARGET_DIR
#
# Picks up where it stopped: a film already there is left alone, and anything
# made is made in TARGET_DIR/.building, which a library does not read, and
# moved out whole. What only serves to make the others is kept in
# TARGET_DIR/.sources. Needs curl, python3 and iconv.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ffmpeg="$1"
target="$2"
building="${target}/.building"
sources="${target}/.sources"
mkdir -p "$building" "$sources"

# The lines of a list file that are neither empty nor comments.
lines_of() {
  grep -v -e '^[[:space:]]*#' -e '^[[:space:]]*$' "$1"
}

# Fetches one address to a file and checks it, keeping nothing that does not
# match. Over HTTP/1.1: Blender's server cut a large download short over
# HTTP/2.
fetch() {
  local url="$1" sum="$2" into="$3"
  curl --fail --location --silent --show-error --retry 3 --http1.1 --output "$into" "$url"
  if [[ "$(sha256sum "$into" | cut -d' ' -f1)" != "$sum" ]]; then
    rm -f "$into"
    echo "${url} does not match its SHA-256" >&2
    return 1
  fi
}

# Every film of `downloads`, downloaded, checked and unpacked where it is a
# zip.
download_all() {
  local name sum url partial
  while IFS='|' read -r name sum url; do
    [[ -e "${target}/${name}" ]] && continue
    echo "downloading ${name}"
    partial="${building}/download"
    fetch "$url" "$sum" "$partial"
    if [[ "$url" == *.zip ]]; then
      python3 - "$partial" "${building}/unpacked" <<'UNZIP'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as archive:
    films = [entry for entry in archive.namelist() if not entry.endswith("/")]
    with archive.open(films[0]) as film, open(sys.argv[2], "wb") as out:
        while chunk := film.read(1 << 20):
            out.write(chunk)
UNZIP
      rm -f "$partial"
      partial="${building}/unpacked"
    fi
    mkdir -p "$(dirname "${target}/${name}")"
    mv "$partial" "${target}/${name}"
  done < <(lines_of "${here}/downloads")
}

# Runs the tool quietly into the building folder, then moves the result out
# under its name. Nothing half made ever shows in the library.
make_film() {
  local name="$1"
  shift
  [[ -e "${target}/${name}" ]] && return 0
  echo "making ${name}"
  local partial="${building}/${name}"
  "$ffmpeg" -hide_banner -nostdin -loglevel error -y "$@" "$partial"
  mv "$partial" "${target}/${name}"
}

# A file that is not a film: written by a command into the building folder,
# then moved out the same way.
make_file() {
  local name="$1"
  shift
  [[ -e "${target}/${name}" ]] && return 0
  echo "making ${name}"
  "$@" "${building}/${name}"
  mv "${building}/${name}" "${target}/${name}"
}

download_all

sintel="${target}/Sintel (2010).mkv"
hdr_4k="${target}/Melyxar test - 4K HDR10 HEVC.mp4"
hdr_1080="${target}/Melyxar test - 1080p HDR10 HEVC.mp4"

# Three minutes of Sintel, a minute in, where the films made from it start.
excerpt=(-ss 60 -t 180 -i "$sintel")

# Lines a picture subtitle shows over a film of a given length, one every four
# seconds, every third one at the top of the picture.
picture_lines() {
  local seconds="$1" at=1 number=1 place
  while ((at + 3 <= seconds)); do
    place="bottom"
    ((number % 3 == 0)) && place="top"
    printf '%s|%s|%s|Picture subtitle %s\\nwith accents: é, à, ç, œ\n' \
      "$at" "$((at + 3))" "$place" "$number"
    at=$((at + 4))
    number=$((number + 1))
  done
}

# How long a film is, in whole seconds.
seconds_of() {
  local probe="${ffmpeg%ffmpeg}ffprobe"
  "$probe" -v error -show_entries format=duration -of csv=p=0 "$1" | cut -d. -f1
}

# A picture subtitle as large as the film it goes with.
picture_subtitle() {
  local film="$1" into="$2" size
  size="$("${ffmpeg%ffmpeg}ffprobe" -v error -select_streams v:0 \
    -show_entries stream=width,height -of csv=p=0:s=x "$film")"
  picture_lines "$(seconds_of "$film")" |
    python3 "${here}/pgs.py" "$ffmpeg" "${size%x*}" "${size#*x}" "$into"
}

# Picture subtitles: inside a 4K HDR10 film, which must then be converted and
# painted at once, and beside a 1080p one, as a file of their own.
with_picture_subtitle() {
  picture_subtitle "$hdr_4k" "${building}/4k.sup"
  "$ffmpeg" -hide_banner -nostdin -loglevel error -y -i "$hdr_4k" -i "${building}/4k.sup" \
    -map 0 -map 1 -c copy -metadata:s:s:0 language=fre -disposition:s:0 default "$1"
  rm -f "${building}/4k.sup"
}
make_file "Melyxar test - 4K HDR10 with picture subtitles.mkv" with_picture_subtitle
make_file "Melyxar test - 1080p HDR10 HEVC.fr.sup" picture_subtitle "$hdr_1080"

# Styled subtitles, every line something a plain subtitle cannot do.
make_film "Melyxar test - styled subtitles.mkv" \
  -ss 60 -t 70 -i "$sintel" -i "${here}/styled.ass" -map 0:v -map 0:a -map 1 \
  -c copy -metadata:s:s:0 language=eng

# One of Sintel's own subtitle tracks, over the same three minutes, as a file.
# Cut on the way out too: a subtitle read from a film is not stopped where the
# film's excerpt is.
sintel_subtitle() {
  "$ffmpeg" -hide_banner -nostdin -loglevel error -y "${excerpt[@]}" \
    -map "0:s:m:language:$1" -t 180 "$2"
}

# Forced subtitles, inside and beside: only some lines, shown whatever the
# viewer chose.
make_file "Melyxar test - forced subtitles.fr.forced.srt" sintel_subtitle fre
make_film "Melyxar test - forced subtitles.mkv" \
  "${excerpt[@]}" -map 0:v -map 0:a -map 0:s:m:language:eng -map 0:s:m:language:fre -c copy \
  -disposition:s:0 0 -disposition:s:1 forced

# External subtitles, one of them in the old Western European encoding rather
# than UTF-8, as many files found on the internet still are.
make_film "Melyxar test - external subtitles.mkv" "${excerpt[@]}" -map 0:v -map 0:a -c copy
make_file "Melyxar test - external subtitles.en.srt" sintel_subtitle eng
latin() {
  sintel_subtitle fre "${building}/utf8.srt"
  iconv -f UTF-8 -t ISO-8859-1//TRANSLIT "${building}/utf8.srt" > "$1"
  rm -f "${building}/utf8.srt"
}
make_file "Melyxar test - external subtitles.fr.srt" latin

# Sound a browser cannot play, to be converted.
make_film "Melyxar test - TrueHD 5.1.mkv" \
  "${excerpt[@]}" -map 0:v -map 0:a -c:v copy -c:a truehd -strict experimental \
  -metadata:s:a:0 title="TrueHD 5.1"
make_film "Melyxar test - DTS 5.1.mkv" \
  "${excerpt[@]}" -map 0:v -map 0:a -c:v copy -c:a dca -strict experimental \
  -metadata:s:a:0 title="DTS 5.1"

# Several sound tracks: the original, a French stereo one and a commentary.
make_film "Melyxar test - several sound tracks.mkv" \
  "${excerpt[@]}" -map 0:v -map 0:a -map 0:a -map 0:a -c:v copy \
  -c:a:0 copy -metadata:s:a:0 language=eng -metadata:s:a:0 title="Original 5.1" \
  -c:a:1 aac -ac:a:1 2 -metadata:s:a:1 language=fre -metadata:s:a:1 title="French stereo" \
  -c:a:2 libopus -ac:a:2 2 -b:a:2 96k -metadata:s:a:2 language=eng \
  -metadata:s:a:2 title="Commentary" -disposition:a:2 comment

# Chapters every thirty seconds, to start in the middle of one.
chapters() {
  printf ';FFMETADATA1\n'
  local at
  for at in 0 30 60 90 120 150; do
    printf '[CHAPTER]\nTIMEBASE=1/1\nSTART=%s\nEND=%s\ntitle=Chapter %s\n' \
      "$at" "$((at + 30))" "$((at / 30 + 1))"
  done
}
chapters > "${sources}/chapters.txt"
make_film "Melyxar test - chapters.mkv" \
  "${excerpt[@]}" -i "${sources}/chapters.txt" -map 0:v -map 0:a -map_chapters 1 -c copy

# Interlaced pictures: broadcast television, and a DVD in 4:3 with the black
# bars in the picture.
make_film "Melyxar test - interlaced 1080i MPEG-2.ts" \
  "${excerpt[@]}" -map 0:v -map 0:a \
  -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:-1:-1,fps=50,interlace" \
  -c:v mpeg2video -b:v 15M -maxrate 20M -bufsize 10M -flags +ilme+ildct \
  -c:a ac3 -b:a 384k
make_film "Melyxar test - DVD 576i 4-3.mpg" \
  "${excerpt[@]}" -map 0:v -map 0:a \
  -vf "scale=720:-2,pad=720:576:-1:-1,fps=50,interlace,setsar=16/15" -aspect 4:3 \
  -target pal-dvd -flags +ilme+ildct -ac 2

# Frame rates other than the film's own, and one that changes as it goes.
make_film "Melyxar test - 25 fps.mkv" \
  "${excerpt[@]}" -map 0:v -map 0:a -vf fps=25 -c:v libx264 -preset veryfast -crf 20 -c:a copy
make_film "Melyxar test - 29.97 fps.mkv" \
  "${excerpt[@]}" -map 0:v -map 0:a -vf fps=30000/1001 -c:v libx264 -preset veryfast -crf 20 \
  -c:a copy
make_film "Melyxar test - variable frame rate.mp4" \
  "${excerpt[@]}" -map 0:v -map 0:a \
  -vf "select='not(between(mod(t\,10)\,4\,7)*mod(n\,3))'" -fps_mode vfr \
  -c:v libx264 -preset veryfast -crf 20 -c:a aac -ac 2

# Shapes: black bars written into a 16:9 picture, and a 4:3 picture.
make_film "Melyxar test - black bars in the picture.mp4" \
  "${excerpt[@]}" -map 0:v -map 0:a -vf "pad=1920:1080:-1:-1" \
  -c:v libx264 -preset veryfast -crf 20 -c:a aac -ac 2
make_film "Melyxar test - 4-3.mp4" \
  "${excerpt[@]}" -map 0:v -map 0:a -vf "crop=1088:816" \
  -c:v libx264 -preset veryfast -crf 20 -c:a aac -ac 2

# An old film: MPEG-4 Part 2 and MP3 in an AVI.
make_film "Melyxar test - old AVI.avi" \
  "${excerpt[@]}" -map 0:v -map 0:a -vf scale=1280:-2 -c:v mpeg4 -vtag XVID -q:v 4 \
  -c:a libmp3lame -ac 2 -b:a 192k

# A film cut off partway, as an interrupted download leaves it.
truncated() {
  "$ffmpeg" -hide_banner -nostdin -loglevel error -y "${excerpt[@]}" \
    -map 0:v -map 0:a -c copy "${building}/whole.mkv"
  head -c "$(($(stat -c %s "${building}/whole.mkv") * 3 / 5))" "${building}/whole.mkv" > "$1"
  rm -f "${building}/whole.mkv"
}
make_file "Melyxar test - cut off partway.mkv" truncated

# Three hours, small, made of two minutes played over and over.
long() {
  "$ffmpeg" -hide_banner -nostdin -loglevel error -y -ss 60 -t 120 -i "$sintel" \
    -map 0:v -map 0:a -vf scale=-2:480 -c:v libx264 -preset veryfast -b:v 600k \
    -c:a aac -ac 2 -b:a 96k "${building}/piece.mkv"
  "$ffmpeg" -hide_banner -nostdin -loglevel error -y -stream_loop 89 -i "${building}/piece.mkv" \
    -c copy "$1"
  rm -f "${building}/piece.mkv"
}
make_file "Melyxar test - three hours.mkv" long

rm -rf "$building"
echo "done"
