# 코드체인 TPS 측정

[코드체인](https://github.com/CodeChain-io/codechain)은 [텐더민트](https://tendermint.com/static/docs/tendermint.pdf)기반의 PoS 블록체인으로, 빠른 블록생성 속도와 비잔틴 장애 허용을 지닌 강력한 컨센서스를 제공합니다.
본 문서는 임의로 구성한 코드체인 체인의 TPS(Transactions per second)를 측정하는 방법을 설명합니다.

## 빌드

코드체인 노드를 실행하기 위해서는 클라이언트 실행파일을 Linux에서 실행해야합니다.
빌드를 하지 않고 제공된 바이너리를 바로 사용하실 경우에는 다음 링크에서 다운로드 하실 수 있습니다. [링크](www.naver.com)

1. 공식 GitHub 레포지터리의 `tps-cleanup` 브랜치를 `clone`합니다. [링크](https://github.com/CodeChain-io/codechain/tree/tps-cleanup)
2. `gcc`, `g++`, `make`를 설치합니다.
3. Rust 1.37.0을 설치합니다. [rustup](https://rustup.rs/)을 권장합니다.
4. `cargo build --release`를 실행하면 `target/release/codechain`이 생성됩니다.

## 머신 구성하기

TPS를 측정하는 시나리오를 실행하기 위해 코드체인 노드를 돌릴 4개의 머신과 스크립트를 실행할 1개의 머신이 필요합니다.
한 컴퓨터에서 5개를 전부 수행해도 되지만 본 메뉴얼은 실제 네트워크를 통해 블록체인의 통신을 살펴보기 위해 AWS에서 제공하는 [EC2](https://aws.amazon.com/ec2/) 클라우드 컴퓨터를 직접 사용하는 것을 기준으로 작성되었습니다.

1. Ubuntu Server 18.04 LTS를 사용하는 `t3.medium` 인스턴스를 4개 생성합니다.
2. SSH로 접속이 가능한걸 확인합니다.
3. AWS 웹페이지에서 해당 인스턴스들이 사용중인 Network Security Group의 설정으로 들어가, inbound rules에서 TCP를 모든 포트, 모든 소스에 대해 허용합니다.

## 코드체인 실행 환경 설정하기

각 머신의 `~/codechain`안에 바이너리와 필요한 파일을 위치시킵니다. 바이너리는 공통이지만 그외 파일들은 4개가 각각 다르니 주의하시길 바랍니다.
- [머신0](www.naver.com)
- [머신1](www.naver.com)
- [머신2](www.naver.com)
- [머신3](www.naver.com)

### 예시

```
Me@node0:~/$ ls
codechain keys password0.json tendermint-tps.json
```

## 테스트 환경 설정하기

코드체인 노드와 별개로 테스트 시나리오를 실행하는 스크립트는 `codechain-sdk`를 사용하여 독립적으로 실행할 수 있습니다.
실행하기 위해 node.js와 어쩌구 저쩌구 **수정필요**


## 네트워크 주소 설정하기

AWS 머신에서 제공하는 공인 IP를 `config${node_num}.toml`과 테스트 스크립트에 직접 기입하여야합니다.

### toml

각각의 머신의 `config${node_num}.toml` 파일을 열면 `bootstrap_addresses` 항목이 있습니다. 이 항목은 코드체인 초기 네트워크 구성시에 서로 연결할 노드를 설정합니다.
**자신을 제외한 다른 세 노드**의 네트워크 주소를 명시하고 있는데, aws에서 제공하는 공인 ip로 바꾸고 포트는 그대로 `:3487`로 유지하면 됩니다.

### 테스트 스크립트

`quick-remote.ts`의 16번째 라인에 위치한 

```ts
const rpcServers = [
    "http://123.123.123.1:2487",
    "http://123.123.123.2:2487",
    "http://123.123.123.3:2487",
    "http://123.123.123.4:2487"
    ];
```
를 각 머신의 공인 ip로 바꿉니다. 포트 `:2487`은 유지합니다.

## 테스트 실행방법

위의 준비과정을 모두 끝내셨으면 이제 테스트를 실행할 수 있습니다.

1. 코드체인 클라이언트를 실행할 각 머신에서 다음 명령어를 입력합니다. `RUST_LOG=error ./codechain --chain tendermint-tps.json --config config${node_num}.toml`. `{node_num}`를 치환하는걸 잊지마십시오.
2. 테스트를 실행할 머신에서는 `/test` 디렉토리로 이동 후에 다음 명령어를 입력합니다. `NODE_ENV=production yarn ts-node src/tendermint.test/quick-remote.ts`
3. 테스트를 실행중인 머신에서 콘솔창에 진행상황을 표시합니다. 총 1,600,000개의 트랜잭션을 전부 실행할 때 까지 반복됩니다.

만약에 테스트를 다시 돌려야 한다면 이전 테스트에서 생성된 데이터베이스를 삭제해야합니다.

1. 각 노드들의 코드체인 프로세스를 종료시킵니다.
2. `rm -rf db`를 실행하여 `db/` 디렉토리를 삭제합니다.

#### tmuxinator 를 활용하는 방법

1. `val${node_num}.yml` 은 코드체인을 적절한 명령줄 인자와 함께 실행시켜줄 `tmuxinator` 의 `session` 시작 파일입니다.
1. `sudo apt install tmux` 와 `sudo apt install tmuxinator` 를 통해서 `tmuxinator`를 설치합니다.
1. `~/.tmuxinator` 디렉토리를 생성하고 `val${node_num}.yml`  파일을 방금 만든 디렉토리 안에 위치시킵니다.
1. `tmuxinator start val${node_num}`의 명령어를 통해서 실행시킵니다.

### 결과

테스트를 실행하면 트랜잭션을 실시간으로 전송한 로그, 새로 업데이트된 블록의 정보를 보여주는 로그, 그리고 현재까지의 통계를 보여주는 로그 간헐적으로 표시됩니다.
이중 `<STATUS>`와 함께 출력되는 정보가 현재까지의 통계에 해당합니다.

```
<STATUS>
Total Consumed: 1166596
Total Elapsed: 463100
TPS: 2519.1017058950547
```
이 경우에는 현재까지 총 실행된 (체인에 포함된) 트랜잭션의 수가 1,166,596개이며 소모된 시간은 463,100 밀리초, 그리고 TPS는 2519txs/s임을 뜻합니다.
1,600,000개가 전부 실행되고 스크립트가 종료되기 직전에 표시된 `<STATUS>`가 최종결과가 되겠습니다.

### 실험

위와 똑같은 AWS머신을 사용하고 같은 절차를 밟아 내부적으로 테스트를 수행해본 결과는 다음과 같습니다.

- 총 실행된 트랜잭션 수: 1,600,000개
- 총 소요된 시간: 663,685 밀리초
- TPS: 2410.782 txs/s