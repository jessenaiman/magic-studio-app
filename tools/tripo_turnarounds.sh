#!/usr/bin/env bash
# Generate a turnaround sheet per character in Tripo's Image tab (GPT Image 2.5), paced to avoid bot checks.
# usage: tripo_turnarounds.sh name [name...]
set -u
export AGENT_BROWSER_SESSION=tripo
A=/run/media/jesse/Speedy/magic/assets/characters
J=$A/farm-school/character-prompts.json
O=$A/farm-school/turnarounds
ab(){ agent-browser --profile Default --headed "$@"; sleep 3; }
snap(){ agent-browser --profile Default --headed snapshot -i -c 2>&1; }
ref(){ snap | grep -o "$1 \[ref=e[0-9]*" | head -1 | grep -o 'e[0-9]*$'; }
for n in "$@"; do
  P=$(python3 -c "import json;print(json.load(open('$J'))['$n']['turnaround_prompt'])")
  img=$A/tripo-input/approved/$n.png
  ab press Escape >/dev/null
  i=$(ref 'link "Image"'); [ -n "$i" ] && ab click @$i >/dev/null
  agent-browser --profile Default --headed eval "[...document.querySelectorAll('[class*=\"i-tripo:delete\"]')].filter(e=>e.getBoundingClientRect().x<330&&e.getBoundingClientRect().y<400).forEach(d=>(d.closest('button')||d).click())" >/dev/null; sleep 2
  ab upload "$(ref 'button "Choose Files"' | sed 's/^/@/')" "$img" >/dev/null
  tb=$(snap | grep -o 'textbox "Create images[^"]*" \[ref=e[0-9]*' | grep -o 'e[0-9]*$'); ab fill @$tb "$P" >/dev/null
  before=$(agent-browser --profile Default --headed eval "[...document.querySelectorAll('img')].filter(i=>i.src.includes('generate-image')).map(i=>i.src.split('?')[0])[0]||''" | tr -d '"')
  ab click @$(ref 'button "Generate Image"') >/dev/null
  for k in $(seq 1 40); do
    sleep 8
    now=$(agent-browser --profile Default --headed eval "[...document.querySelectorAll('img')].filter(i=>i.src.includes('generate-image')).map(i=>i.src)[0]||''" | tr -d '"')
    [ -n "$now" ] && [ "${now%%\?*}" != "$before" ] && break
  done
  curl -s -o "$O/$n.png" "$now" && python3 /run/media/jesse/Speedy/magic/tools/split_white_turnaround.py "$O/$n.png" "$O/$n" >/dev/null && echo "OK $n" || echo "FAIL $n"
  sleep 5
done
