// Copyright 2019 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

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
