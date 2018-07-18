// Copyright 2018 Kodebox, Inc.
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

use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::parcel_invoice::ParcelInvoice;

#[derive(Clone, Debug, PartialEq)]
pub struct BlockInvoices {
    pub invoices: Vec<ParcelInvoice>,
}

impl BlockInvoices {
    pub fn new(invoices: Vec<ParcelInvoice>) -> Self {
        Self {
            invoices,
        }
    }
}

impl Decodable for BlockInvoices {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let invoices = rlp
            .as_list::<Vec<u8>>()?
            .iter()
            .map(|parcel_invoice| UntrustedRlp::new(&parcel_invoice).as_val::<ParcelInvoice>())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            invoices,
        })
    }
}

impl Encodable for BlockInvoices {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(self.invoices.len());
        for i in self.invoices.iter() {
            let encoded = i.rlp_bytes();
            s.append(&encoded.into_vec());
        }
    }
}


#[cfg(test)]
mod tests {
    use super::super::invoice::Invoice;

    use super::*;

    #[test]
    fn rlp_encode_and_decode_block_invoices() {
        let invoices = vec![Invoice::Success, Invoice::Failed];
        let parcel_invoice = ParcelInvoice::new(invoices);
        rlp_encode_and_decode_test!(BlockInvoices {
            invoices: vec![
                parcel_invoice.clone(),
                parcel_invoice.clone(),
                parcel_invoice.clone(),
                parcel_invoice.clone(),
            ],
        });
    }
}
