#!/bin/bash
# usage: render.sh populated|template   (builds .dcs -> .designcraft -> pngs + pdf)
set -e; cd "$(dirname "$0")"; D=/home/jesse/.local/bin/designcraft-cli
m=$1; n=nelly-storyboard; [ "$m" = template ] && n=studio-template
python3 build_storyboard.py $m
$D script $n.dcs --save $n.designcraft > /tmp/$n.run.json
mkdir -p renders/$n; rm -f renders/$n/*.png
$D run --in $n.designcraft --scale 1.5 --all-pages renders/$n >/dev/null
$D run --in $n.designcraft --export $n.pdf >/dev/null
