#!/bin/bash

BASEDIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )


LOGFILE="$BASEDIR/mark_log.txt"

ID_FILE="$BASEDIR/mark_id"
ID=$(cat "$ID_FILE")
ID=$((ID + 1))

# initialize if file doesn't exist
if [ ! -f "$ID_FILE" ]; then
    echo 0 > "$ID_FILE"
fi

TS=$(date +%s)

for dev in /dev/ttyUSB*; do
    if [ -e "$dev" ]; then
        echo "Sending MARK to $dev"
        echo "MARK $ID $TS" > "$dev"
    fi
done

echo "MARK $ID $TS"

#xdotool type "MARK $ID $TS"
#xdotool key Return

NAME="$1"
echo "MARK $NAME $ID $TS" >> "$LOGFILE"
echo "MARK $NAME $ID $TS"
echo "$ID" > "$ID_FILE"
