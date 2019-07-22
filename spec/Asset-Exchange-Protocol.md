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

|        Name        |    Data Type    |                                                Description                                                |
|--------------------|-----------------|-----------------------------------------------------------------------------------------------------------|
| assetTypeFrom      | H160            | The type of the asset offered by maker                                                                    |
| assetTypeTo        | H160            | The type of the asset requested by maker                                                                  |
| assetTypeFee       | H160            | The type of the asset offered by maker to give as fees                                                    |
| shardIdFrom        | ShardId         | The shard ID of the asset offered by maker                                                                    |
| shardIdTo          | ShardId         | The shard ID of the asset requested by maker                                                                  |
| shardIdFee         | ShardId         | The shard ID of the asset offered by maker to give as fees                                                    |
| assetQuantityFrom    | U64             | Total quantity of assets with the type assetTypeFrom                                                        |
| assetQuantityTo      | U64             | Total quantity of assets with the type assetTypeTo                                                          |
| assetQuantityFee     | U64             | Total quantity of assets with the type assetTypeFee                                                         |
| originOutputs      | AssetOutPoint[] | The previous outputs composed of assetTypeFrom / assetTypeFee assets, which the order starts from         |
| expiration         | U64             | Time at which the order expires                                                                           |
| lockScriptHashFrom | H160            | Lock script hash provided by maker, which should be written in every output with the order except fee     |
| parametersFrom     | Bytes[]         | Parameters provided by maker, which should be written in every output with the order except fee           |
| lockScriptHashFee  | H160            | Lock script hash provided by relayer, which should be written in every fee output with the order          |
| parametersFee      | Bytes[]         | Parameters provided by relayer, which should be written in every fee output with the order                |

To make a point-to-point order, put a zero on the `assetQuantityFee` field.
To write an order on a transfer transaction, the order should be wrapped once more, to `OrderOnTransfer`.

## OrderOnTransfer format

If there are inputs and outputs with the same order, it is wasteful to put the order in every input/output. Therefore, orders are wrapped into `OrderOnTransfer`.

| Name                        | Data Type | Description                                                                      |
| --------------------------- | --------- | -------------------------------------------------------------------------------- |
| order                       | Order     | The order to write on the transfer transaction                                   |
| spentQuantity               | U64       | The spent quantity of `assetTypeFrom` of the order in the transfer transaction   |
| inputFromIndices            | Index[]   | The indices of the transfer inputs that are protected by the order ( assetFrom) |
| inputFeeIndices             | Index[]   | The indices of the transfer inputs that are protected by the order (assetFee)   |
| outputFromIndices           | Index[]   | The indices of the transfer outputs that are protected by the order (assetFrom) |
| outputToIndices             | Index[]   | The indices of the transfer outputs that are protected by the order (assetTo)   |
| outputOwnedFeeIndices       | Index[]   | The indices of the transfer outputs that are protected by the order (assetFee)  |
| outputTransferredFeeIndices | Index[]   | The indices of the transfer outputs that are protected by the order (assetFee)  |

And the format of transfer transaction is as shown below.

|      Name     |       Data Type       |
|---------------|-----------------------|
| networkId     | NetworkId             |
| burns         | AssetTransferInput[]  |
| inputs        | AssetTransferInput[]  |
| outputs       | AssetTransferOutput[] |
| orders        | OrderOnTransfer[]     |

## How to support partial fills?

As described above in the 10-gold-to-100-silver order, it is possible to make a transfer transaction which has a 10-gold input that results in a 5-gold and 50-silver output, tagged with the same order. (Other inputs/outputs should be provided by a taker or a relayer). After this transaction, an asset contains the hash of the *consumed order*, which is a 5-gold-to-50-silver order, not a 10-gold-to-100-silver order. To use the 5-gold output, provide the 5-gold-to-50-silver order with the same information except for the `assetQuantityFrom` and the `assetQuantityTo` field. Neither a lock script nor an unlock script is needed for the 5-gold input. CodeChain will compare the order of an input and the order of the corresponding previous output, and will not run VM on the order if those orders are identical.

## How to support cancellation?

An order can be cancelled by the maker before it is completely filled. Let's use the described 10-gold-to-100-silver order example again. Suppose the maker wants to cancel the order after the 5-gold and 50-silver transaction is processed. Unlike the previous partial fill case, this time the maker has to prove the ownership of the remaining asset. Both a lock script and an unlock script is needed for the 5-gold input. Writing a new transaction with empty `orders` field with this input makes CodeChain cancel the remaining 5-gold and 50-silver order.

## How to handle matched two orders with different exchange ratios?

Suppose Alice and Bob want to exchange their own assets through a DEX platform. Alice offered a 10-gold-to-100-silver exchange ratio but Bob offered a 5-gold-to-100-silver exchange ratio, which has higher gold to silver ratio compared to the Alice's offer. For DEX platforms to satisfy both Alice and Bob, the minimum conditions are "Alice gets at least 100 silvers at the expense of 5 golds" and "Bob gets at least 5 golds at the expense of 100 silvers". Satisfying these conditions, there are extra 5 golds. Because CodeChain only checks the minimum conditions, DEX platforms have a freedom of how they dispose these extra assets. As long as the minimum conditions are satisfied, any possible choice of transaction could be accepted.