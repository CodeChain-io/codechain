# Tag encoding

![img](https://cdn-images-1.medium.com/max/880/0*YripqzIkBK6EoNLz) 

The tag consists of one byte of prefix and a dynamic length output filter. First, the one-byte prefix includes the output scheme bit, input scheme bit and output filter length. The tag should follow the rules below. Not doing so will make the instruction fail.

1. The filter length has to be zero if the output scheme is one.

2. 6bit of filter length is the same as the byte length of the output filter following it.

3. Trailing zero bytes are not allowed. If it exists, the instruction will fail. (for example, `0x0000000011010100` is an invalid output filter)

After confirming that the inserted tag follows above rules, signing input proceeds. If the input scheme bit is zero, then only the executing input is signed by ECDSA secret key, which is owned by the user. On the other hand, if the input scheme bit is one, then all of the inputs in the transaction are signed. The output scheme bit is interpreted in a similar way, but a little differently when it is zero. When the output scheme bit is one, all of the outputs in the transaction will be signed. In contrast, if the output scheme bit is zero,  a special filtering rule will be applied to outputs in the transaction. Let's see the output filter as a bit array in which each bit has a number sequentially. Each bit is matched with the output's index, so if the matched bit is zero, then the output will be signed; otherwise, it will not be signed.

