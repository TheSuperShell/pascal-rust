#!/bin/sh

if [[ $1 -gt 1 ]]; then 
    echo "will run $1 times"
else
    echo "first arg should be a number > 1"
    exit 1
fi

echo "warmup"
$2 >/dev/null
$2 >/dev/null
$2 >/dev/null
echo "start"

start=$(($(date +%s%N)/1000000))

count=1
while [ $count -le $1 ]; do
    $2 >/dev/null
    ((count++))
done

end=$(( $(date +%s%N)/1000000 - $start ))
echo "finished in $end milliseconds"
echo "avg: $(( $end/$1 )) milliseconds"
