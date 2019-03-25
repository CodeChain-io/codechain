import { Mock } from "../";

async function sendBlock() {
    const mock = new Mock("0.0.0.0", 3485, "tc");
    mock.setLog();
    await mock.establish();
    // Genesis block
    const header = mock.soloGenesisBlockHeader();

    // Block 1
    const header1 = mock.soloBlock1(header.hashing());

    // Block 2
    const header2 = mock.soloBlock2(header1.hashing());

    await mock.sendEncodedBlock(
        [header.toEncodeObject(), header1.toEncodeObject(), header2.toEncodeObject()],
        [[], []],
        header2.hashing(),
        header2.getScore()
    );

    await mock.end();
}

sendBlock();
