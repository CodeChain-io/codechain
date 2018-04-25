#!/bin/sh

if [ $# -eq 0 ]; then
    echo "USAGE :"
    echo "    test NUM_CLIENTS [OPTIONS]"
    exit 1
fi

NUM_CLIENTS=$1
OTHER_OPTIONS=${@:2}

BASE_DIR=$(cd "$(dirname "$0")"/.. && pwd)
LOG_DIR=${BASE_DIR}/log
DB_DIR=${BASE_DIR}/db

CODECHAIN_PORT_START=3484
RPC_PORT_START=8080

mkdir -p ${LOG_DIR}
mkdir -p ${DB_DIR}

run_server() {
    if [ $1 -eq 0 ]; then
        BOOTSTRAP=""
    else
        BOOTSTRAP="--bootstrap-addresses 127.0.0.1:${CODECHAIN_PORT_START}"
    fi
    cd ${BASE_DIR}
    cargo run -- \
        --db-path ${DB_DIR}/db$1 \
        --port $((${CODECHAIN_PORT_START} + $1)) \
        --jsonrpc-port $((${RPC_PORT_START} + $1)) \
        --secret-key "`printf "%064x" $(($1 + 1))`" \
        ${BOOTSTRAP} \
        ${OTHER_OPTIONS} \
    > ${LOG_DIR}/codechain.log.$1 2>&1 &
}

echo "Building..."
cd ${BASE_DIR}
cargo build

run_server 0

echo "Waiting for startup...."

if [ -x /usr/bin/expect ]; then
    /usr/bin/expect <<EOD
spawn tail -f ${LOG_DIR}/codechain.log.0
expect "TCP connection starts for"
EOD
else
    tail -f ${LOG_DIR}/codechain.log.0 | grep -qm 1 "Initialization complete"
fi

for i in `seq 1 $((${NUM_CLIENTS} - 1))`; do
    run_server $i
done

echo ""
echo "Running ${NUM_CLIENTS} clients"
echo "DB location : ${DB_DIR}/db*"
echo "Log : ${LOG_DIR}/codechain.log.*"
echo "Codechain port on ${CODECHAIN_PORT_START}..$((${CODECHAIN_PORT_START} + ${NUM_CLIENTS} - 1))"
echo "RPC port on ${RPC_PORT_START}..$((${RPC_PORT_START} + ${NUM_CLIENTS} - 1))"
echo ""

trap 'kill -9 `pidof codechain`' INT
cat ${LOG_DIR}/codechain.log.0
tail -f ${LOG_DIR}/codechain.log.0
