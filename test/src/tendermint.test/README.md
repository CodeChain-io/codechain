## 코드체인 노드 구성하기

1. 각각의 `val${node_num}` 압축파일은 서로 다른 머신에 분배되어야 한다.
2. `CodeChain` 폴더는 디렉토리 구조를 그대로 살려서, 각 머신의 임의의 사용자 홈폴더에 위치시킨다.
3. `val${node_num}.yml` 은 코드체인을 적절한 명령줄 인자와 함께 실행시켜줄 `tmuxinator` 의 `session` 시작 파일이다.

### 네트워크 설정 구성하기

1. 각각의 `CodeChain` 폴더 안에 있는 `config${node_num}.toml` 파일을 열면 `bootstrap_addresses` 항목이 있다. 이 항목은 코드체인 초기 네트워크 구성시에 서로 연결할 노드를 설정한다.
2. 설정을 진행하는 머신을 제외한 나머지 3대의 머신의 `${퍼블릭 IP주소}:${포트번호}` 를 이 항목에 배열로 나열한다. (p2p 연결의 포트번호와 json-rpc의 포트번호는 모두 같은 파일에서 수정할 수 있다)
3. 모든 4개의 노드의 설정에 대해서 자신을 제외한 세 개의 노드를 `bootstrap_addresses`에 적어주면 된다.
4. 각 노드의 서버에는 포트 2487 와 3487번을 `Inbound` 허용해주는 설정을 해야한다.

### 코드체인 실행시키기

#### tmuxinator 를 활용하는 방법

1. `sudo apt install tmux` 와 `sudo apt install tmuxinator` 를 통해서 `tmuxinator`를 설치한다.
2. `~/.tmuxinator` 디렉토리를 생성하고 `val${node_num}.yml`  파일을 방금 만든 디렉토리 안에 위치시킨다.
3. `tmuxinator start val${node_num}`의 명령어를 통해서 실행시킨다.

#### 명령줄에서 실행하는 방법

1. 홈폴더에서 `cd ./codechain`
2. `RUST_LOG=error ./codechain --chain tendermint-tps.json --config config${node_num}.toml`

### 코드체인 노드 리셋하기

TPS 측정에서 사용될 트랜잭션들은 모두 처리 가능한 트랜잭션들이다. 하지만 중간에 측정스크립트를 중단하거나 새롭게 측정을 시작할 때에는 기존에 장부에 남겨진 기록들을 지워야 재동작하도록 할 수 있다.

1. 각각의 노드에서 코드체인 프로세스를 종료시킨다.
2. 각각의 노드에서 `cd ~/codechain` 을 통해 코드체인 디렉토리로 이동한다.
3. `rm -rf db`를 통해서 데이터베이스 파일을 지워준다.
4. `코드체인 실행시키기` 에 있던 방법대로 다시 코드체인노드를 켠다.