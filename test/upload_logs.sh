if [ -d "./log" ]; then
    cd ./log
    count=$(git ls-files -o | wc -l)
    echo "Log file list: "
    git ls-files -o

    for (( i=1; i<="$count";i++ )); do
        file=$(echo $(git ls-files -o | sed "${i}q;d"))
        echo "uploading $file"
        echo "uploaded $file at $(curl --upload-file $file https://transfer.sh/)"
    done
fi