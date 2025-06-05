#!/usr/bin/env bash

user=$1
lines="lines=${2-40}L"
page="https://jstris.jezevec10.com/sprint?display=5&$lines&user=$user&page="

f=$user-sprints-$(date -I).txt
rm -f "$f"

# TODO: do you want to refetch? you already have data for that user from <mtime> ago

time=0.00
len=200
cur=0
while [ "$len" -eq 200 ]; do
    len=$(xh "$page$time" | pup 'table tbody tr json{}' |
        jq '.[].children | ((
                .[2].children[] | .text+.children[0].text),
                .[3:6][].text,
               (.[6].text | sub(" "; "T")),
                .[7].children[0].href//"" | split("/") | last)' -r |
        xargs -L6 |
        sd '^(?:(\d+):)?(\d\d(?:\.?\d{1,3})?)' 'echo $(bc <<<"0$1 * 60 + $2")' |
        sh | tee -a "$f" | wc -l)
    longest=$(tail -n1 "$f" | choose 0)
    time=$(bc <<<"$longest + .001")
    ((cur += len))
    echo -e "$cur games \t (up to ${time}s)"
    sleep 1 # TODO: read Retry-After header when 429's happen for better rate limiting
done

sort -k5 "$f" | column -t | sponge "$f"

echo -e "\nrun:\n\ngnuplot -e \"filename='$f'\" sprints.gpi\n"
echo "for pretty graphs. check scripts.gpi for some more options"
