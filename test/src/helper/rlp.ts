export function readOptionalRlp<T>(
    bytes: [] | [Buffer],
    decoder: (b: Buffer) => T
) {
    if (bytes.length === 0) {
        return null;
    } else {
        return decoder(bytes[0]);
    }
}

export function readUIntRLP(bytes: Buffer) {
    if (bytes.length === 0) {
        return 0;
    } else {
        return bytes.readUIntBE(0, bytes.length);
    }
}
