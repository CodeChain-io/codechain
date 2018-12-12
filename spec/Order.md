CodeChain supports an on-chain order to support DEX(Decentralized Exchange).

## Order

An order can be either a _point-to-point order_ between a maker and a taker, or a _broadcast order_ between a maker, a relayer and takers.
Point-to-point orders allow two parties(makers and takers) to directly exchange assets between each other.
Broadcast orders are similar to point-to-point orders, but also allow relayers to take fee assets from makers and takers. Broadcast orders are usually used in decentralized exchange platforms.

Orders on CodeChain are implemented on UTXO forms.
Basically, an order is like a tag on products in grocery stores. Orders can be put on inputs and outputs of transfer transactions. If an input/output has an order, it means that that specific input/output should be exchanged as its order says. Think about this situation: Let’s say we have an order to exchange 10 gold to 100 silver, and we've put that order on a 10-gold input. Then there should be a 100-silver output with the same order of 10 gold to 100 silver. If there isn’t enough gold, there would perhaps be a 5-gold output and a 50-silver output, both with the same order, or, whatever outputs with the same order while maintaining the order equilibrium.

Assets with orders must be able to be spent by takers or relayers without any permission. But in the CodeChain UTXO form, those parties should provide a lock script and an unlock script to prove the ownership of the assets.  In order to solve the problem, if there are orders on inputs, CodeChain runs VM on **orders**, not *transactions*, if there are orders on inputs. If there are no orders on inputs, CodeChain runs VM on transactions as usual. Partial signing is not allowed on transactions with orders. With this convention, takers and relayers can take the ownership of the assets with orders with unlock scripts which are provided by makers.


## Order format

The format of `Order` is as shown below.

|       Name      |    Data Type    |                                                Description                                                |
|-----------------|-----------------|-----------------------------------------------------------------------------------------------------------|
| assetTypeFrom   | H256            | The type of the asset offered by maker                                                                    |
| assetTypeTo     | H256            | The type of the asset requested by maker                                                                  |
| assetTypeFee    | H256            | The type of the asset offered by maker to give as fees                                                    |
| assetAmountFrom | U64             | Total amount of assets with the type assetTypeFrom                                                        |
| assetAmountTo   | U64             | Total amount of assets with the type assetTypeTo                                                          |
| assetAmountFee  | U64             | Total amount of assets with the type assetTypeFee                                                         |
| originOutputs   | AssetOutPoint[] | The previous outputs composed of assetTypeFrom / assetTypeFee assets, which the order starts from         |
| expiration      | U64             | Time at which the order expires                                                                           |
| lockScriptHash  | H160            | Lock script hash provided by maker, which should be written in every output with the order                |
| parameters      | Bytes[]         | Parameters provided by maker, which should be written in every output with the order                      |

To make a point-to-point order, put a zero on the `assetAmountFee` field.
To write an order on a transfer transaction, the order should be wrapped once more, to `OrderOnTransfer`.

## OrderOnTransfer format

If there are inputs and outputs with the same order, it is wasteful to put the order in every input/output. Therefore, orders are wrapped into `OrderOnTransfer`.

|       Name      |    Data Type    |                                                Description                                                |
|-----------------|-----------------|-----------------------------------------------------------------------------------------------------------|
| order           | Order           | The order to write on the transfer transaction                                                            |
| spentAmount     | U64             | The spent amount of `assetTypeFrom` of the order in the transfer transaction                              |
| inputIndices    | Index[]         | The indices of the transfer inputs which are protected by the order                                       |
| outputIndices   | Index[]         | The indices of the transfer outputs which are protected by the order                                      |

And the format of transfer transaction is as shown below.

|      Name     |       Data Type       |
|---------------|-----------------------|
| networkId     | NetworkId             |
| burns         | AssetTransferInput[]  |
| inputs        | AssetTransferInput[]  |
| outputs       | AssetTransferOutput[] |
| orders        | OrderOnTransfer[]     |

## How to support partial fills?

As described above in the 10-gold-to-100-silver order, it is possible to make a transfer transaction which has a 10-gold input that results in a 5-gold and 50-silver output, tagged with the same order. (Other inputs/outputs should be provided by a taker or a relayer). After this transaction, an asset contains the hash of the *consumed order*, which is 5-gold-to-50-silver order, not 10-gold-to-100-silver order. In order to use the 5-gold output, provide the 5-gold-to-50-silver order with the same information except for the `assetAmountFrom` and the `assetAmountTo` field. Neither a lock script nor an unlock script is needed for the 5-gold input. CodeChain will compare the order of an input and the order of the corresponding previous output, and will not run VM on the order if those orders are identical.