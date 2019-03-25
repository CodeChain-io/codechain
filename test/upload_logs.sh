if [ -d "./log" ]; then
    cd ./log
    count=$(git ls-files -o | wc -l)
    echo "Log file list: "
    git ls-files -o

    for (( i=1; i<="$count";i++ )); do
        file=$(echo $(git ls-files -o | sed "${i}q;d"))
        echo "uploading $file"
        if [ -z $TRANSFER_SH_URL ]; then
          echo "uploaded $file at $(curl --upload-file $file https://transfer.sh/)";
        else
          echo "uploaded $file at $(curl --upload-file $file -u $TRANSFER_SH_USER:$TRANSFER_SH_PASSWORD $TRANSFER_SH_URL)";
        fi
    done
fi
