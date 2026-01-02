# converts raw register base and sub addresses to register ids from the Qorvo SDK
# usage: bash replace_registers_with_ids.sh dw3000_api.h
# outputs code with write and read calls modified to use register ids

# Source - https://stackoverflow.com/a
# Posted by F. Hauri  - Give Up GitHub, modified by community. See post 'Timeline' for change history
# Retrieved 2026-01-02, License - CC BY-SA 4.0

# Source - https://stackoverflow.com/a
# Posted by dogbane, modified by community. See post 'Timeline' for change history
# Retrieved 2026-01-02, License - CC BY-SA 4.0


SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cp "$1" "regids_$1"

cat "$SCRIPT_DIR/registers_base_sub.csv" |
    while read in; do
        echo "Processing: $in"
        regid=$(echo $in | cut -d',' -f1)
        base=$(echo $in | cut -d',' -f2)
        sub=$(echo $in | cut -d',' -f3)
        cat "regids_$1" | sed "s/read(0x0*$base, *0x0*$sub,/read($regid,/g" "$1" | \
            sed "s/write(0x0*$base, *0x0*$sub,/write($regid,/g" "$1" > "regids_$1"

    done
